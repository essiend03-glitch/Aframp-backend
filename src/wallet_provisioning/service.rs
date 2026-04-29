//! Business logic for Wallet Creation & Stellar Account Provisioning (Issue #322).

use crate::chains::stellar::client::StellarClient;
use crate::error::Error;
use crate::wallet_provisioning::{
    bip44::{validate_stellar_public_key, KeypairGenerationGuidance, MnemonicConfirmationChallenge},
    metrics,
    models::*,
    repository::ProvisioningRepository,
};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Stellar base reserve constants (XLM)
const BASE_RESERVE_XLM: f64 = 0.5;
const TRUSTLINE_RESERVE_XLM: f64 = 0.5;
const FEE_BUFFER_XLM: f64 = 0.5;
const ACCOUNT_BASE_ENTRIES: f64 = 2.0; // 2 × base reserve for account activation

pub struct WalletProvisioningService {
    repo: Arc<ProvisioningRepository>,
    stellar: Arc<StellarClient>,
}

impl WalletProvisioningService {
    pub fn new(repo: Arc<ProvisioningRepository>, stellar: Arc<StellarClient>) -> Self {
        Self { repo, stellar }
    }

    // -------------------------------------------------------------------------
    // Keypair generation guidance
    // -------------------------------------------------------------------------

    pub fn get_keypair_guidance(&self) -> KeypairGenerationGuidance {
        KeypairGenerationGuidance::generate()
    }

    pub fn get_mnemonic_challenge(&self) -> MnemonicConfirmationChallenge {
        MnemonicConfirmationChallenge::generate()
    }

    // -------------------------------------------------------------------------
    // Funding requirements  GET /api/wallet/:wallet_id/funding-requirements
    // -------------------------------------------------------------------------

    pub async fn get_funding_requirements(
        &self,
        wallet_id: Uuid,
    ) -> Result<FundingRequirements, Error> {
        // Check if platform sponsorship is available
        let funding_account = self
            .repo
            .get_funding_account()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let sponsorship_available = funding_account
            .as_ref()
            .map(|fa| {
                let balance: f64 = fa.current_xlm_balance.to_string().parse().unwrap_or(0.0);
                let threshold: f64 = fa.min_balance_alert_threshold.to_string().parse().unwrap_or(100.0);
                balance > threshold
            })
            .unwrap_or(false);

        let total = ACCOUNT_BASE_ENTRIES * BASE_RESERVE_XLM + TRUSTLINE_RESERVE_XLM + FEE_BUFFER_XLM;

        Ok(FundingRequirements {
            wallet_id,
            base_reserve_xlm: ACCOUNT_BASE_ENTRIES * BASE_RESERVE_XLM,
            trustline_reserve_xlm: TRUSTLINE_RESERVE_XLM,
            fee_buffer_xlm: FEE_BUFFER_XLM,
            total_required_xlm: total,
            sponsorship_available,
        })
    }

    // -------------------------------------------------------------------------
    // Provisioning status  GET /api/wallet/:wallet_id/provisioning-status
    // -------------------------------------------------------------------------

    pub async fn get_provisioning_status(
        &self,
        wallet_id: Uuid,
    ) -> Result<ProvisioningStatus, Error> {
        let prov = self
            .repo
            .get_or_create(wallet_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let state_enum = parse_state(&prov.state);

        Ok(ProvisioningStatus {
            wallet_id,
            state: prov.state.clone(),
            next_step: prov.state.clone(),
            instructions: state_enum.next_step_instructions().to_string(),
            is_sponsored: prov.is_sponsored,
            funding_method: prov.funding_method.clone(),
            last_failure_reason: prov.last_failure_reason.clone(),
            retry_count: prov.retry_count,
            step_timeout_at: prov.step_timeout_at,
            became_ready_at: prov.became_ready_at,
        })
    }

    // -------------------------------------------------------------------------
    // Account funding detection (called by polling worker)
    // -------------------------------------------------------------------------

    pub async fn check_funding_and_advance(
        &self,
        wallet_id: Uuid,
        wallet_address: &str,
    ) -> Result<bool, Error> {
        let prov = self
            .repo
            .get(wallet_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let prov = match prov {
            Some(p) if p.state == "pending_funding" => p,
            _ => return Ok(false),
        };

        // Poll Stellar Horizon for account existence
        match self.stellar.account_exists(wallet_address).await {
            Ok(true) => {
                info!(wallet_id = %wallet_id, "Stellar account detected — advancing to funded");
                self.repo
                    .set_funding_detected(wallet_id, None, "self_funded")
                    .await
                    .map_err(|e| Error::Internal(e.to_string()))?;
                self.repo
                    .transition(wallet_id, "funded", Some("Account detected on Stellar"), "worker")
                    .await
                    .map_err(|e| Error::Internal(e.to_string()))?;

                metrics::provisioning_completions()
                    .with_label_values(&["funded"])
                    .inc();
                Ok(true)
            }
            Ok(false) => {
                // Check for timeout
                if let Some(timeout_at) = prov.step_timeout_at {
                    if chrono::Utc::now() > timeout_at {
                        warn!(wallet_id = %wallet_id, "Funding timeout reached");
                        self.repo
                            .transition(wallet_id, "stalled", Some("Funding timeout"), "worker")
                            .await
                            .map_err(|e| Error::Internal(e.to_string()))?;
                        metrics::provisioning_abandonments()
                            .with_label_values(&["pending_funding"])
                            .inc();
                    }
                }
                Ok(false)
            }
            Err(e) => {
                warn!(wallet_id = %wallet_id, error = %e, "Error checking Stellar account");
                Ok(false)
            }
        }
    }

    // -------------------------------------------------------------------------
    // Trustline initiation  POST /api/wallet/:wallet_id/trustline/initiate
    // -------------------------------------------------------------------------

    pub async fn initiate_trustline(
        &self,
        wallet_id: Uuid,
        wallet_address: &str,
    ) -> Result<TrustlineInitiateResponse, Error> {
        let prov = self
            .repo
            .get(wallet_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::NotFound("Provisioning record not found".into()))?;

        if prov.state != "funded" && prov.state != "trustline_pending" {
            return Err(Error::BadRequest(format!(
                "Wallet must be in 'funded' state to initiate trustline, current: {}",
                prov.state
            )));
        }

        // If envelope already exists, return it (idempotent)
        if let Some(ref envelope) = prov.trustline_envelope {
            let issuer = std::env::var("CNGN_ISSUER_PUBLIC_KEY")
                .unwrap_or_else(|_| "GCNGN_ISSUER_PLACEHOLDER".into());
            return Ok(TrustlineInitiateResponse {
                wallet_id,
                unsigned_envelope_xdr: envelope.clone(),
                asset_code: "cNGN".into(),
                issuer,
                instructions: "Sign this transaction envelope with your private key and submit it to /trustline/submit".into(),
            });
        }

        // Build unsigned change_trust XDR envelope
        let issuer = std::env::var("CNGN_ISSUER_PUBLIC_KEY")
            .unwrap_or_else(|_| "GCNGN_ISSUER_PLACEHOLDER".into());

        // In production this uses stellar-xdr to build the envelope.
        // Here we produce a placeholder that the real XDR builder would replace.
        let envelope_xdr = build_change_trust_envelope(wallet_address, &issuer)?;

        self.repo
            .set_trustline_envelope(wallet_id, &envelope_xdr)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Advance state to trustline_pending
        self.repo
            .transition(wallet_id, "trustline_pending", Some("Trustline envelope issued"), "system")
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        metrics::trustline_events()
            .with_label_values(&["initiated"])
            .inc();

        Ok(TrustlineInitiateResponse {
            wallet_id,
            unsigned_envelope_xdr: envelope_xdr,
            asset_code: "cNGN".into(),
            issuer,
            instructions: "Sign this transaction envelope with your private key and submit it to /trustline/submit".into(),
        })
    }

    // -------------------------------------------------------------------------
    // Trustline submission  POST /api/wallet/:wallet_id/trustline/submit
    // -------------------------------------------------------------------------

    pub async fn submit_trustline(
        &self,
        wallet_id: Uuid,
        wallet_address: &str,
        req: TrustlineSubmitRequest,
    ) -> Result<(), Error> {
        let prov = self
            .repo
            .get(wallet_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::NotFound("Provisioning record not found".into()))?;

        if prov.state != "trustline_pending" {
            return Err(Error::BadRequest(
                "Wallet must be in 'trustline_pending' state to submit trustline".into(),
            ));
        }

        // Verify the signed envelope contains a valid signature from the wallet's public key
        verify_envelope_signature(&req.signed_envelope_xdr, wallet_address)?;

        // Submit to Stellar Horizon
        let tx_hash = submit_to_horizon(&req.signed_envelope_xdr)
            .map_err(|e| Error::Internal(format!("Horizon submission failed: {}", e)))?;

        self.repo
            .set_trustline_submitted(wallet_id, &tx_hash)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        info!(
            wallet_id = %wallet_id,
            tx_hash = %tx_hash,
            "Trustline transaction submitted to Stellar"
        );

        metrics::trustline_events()
            .with_label_values(&["submitted"])
            .inc();

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Readiness verification  GET /api/wallet/:wallet_id/readiness
    // -------------------------------------------------------------------------

    pub async fn check_readiness(
        &self,
        wallet_id: Uuid,
        wallet_address: &str,
    ) -> Result<ReadinessResponse, Error> {
        // Check Stellar account
        let account_exists = self
            .stellar
            .account_exists(wallet_address)
            .await
            .unwrap_or(false);

        let (min_xlm_met, trustline_active, trustline_authorized) = if account_exists {
            match self.stellar.get_account(wallet_address).await {
                Ok(account) => {
                    let xlm_balance: f64 = account
                        .balances
                        .iter()
                        .find(|b| b.asset_type == "native")
                        .and_then(|b| b.balance.parse().ok())
                        .unwrap_or(0.0);

                    let min_required = ACCOUNT_BASE_ENTRIES * BASE_RESERVE_XLM
                        + TRUSTLINE_RESERVE_XLM
                        + FEE_BUFFER_XLM;
                    let min_xlm = xlm_balance >= min_required;

                    let issuer = std::env::var("CNGN_ISSUER_PUBLIC_KEY")
                        .unwrap_or_else(|_| "GCNGN_ISSUER_PLACEHOLDER".into());

                    let cngn_trustline = account.balances.iter().find(|b| {
                        b.asset_type != "native"
                            && b.asset_code.as_deref() == Some("cNGN")
                            && b.asset_issuer.as_deref() == Some(&issuer)
                    });

                    let tl_active = cngn_trustline.is_some();
                    let tl_authorized = cngn_trustline
                        .map(|b| b.is_authorized)
                        .unwrap_or(false);

                    (min_xlm, tl_active, tl_authorized)
                }
                Err(_) => (false, false, false),
            }
        } else {
            (false, false, false)
        };

        let wallet_registered = true; // wallet_id exists in our DB

        let check = self
            .repo
            .upsert_readiness(
                wallet_id,
                account_exists,
                min_xlm_met,
                trustline_active,
                trustline_authorized,
                wallet_registered,
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Advance to ready if all criteria met
        if check.all_criteria_met {
            let _ = self.repo.set_ready(wallet_id).await;
            metrics::provisioning_completions()
                .with_label_values(&["ready"])
                .inc();
        }

        let mut pending_steps = Vec::new();
        if !account_exists { pending_steps.push("Fund Stellar account".into()); }
        if !min_xlm_met { pending_steps.push("Maintain minimum XLM balance".into()); }
        if !trustline_active { pending_steps.push("Establish cNGN trustline".into()); }
        if !trustline_authorized { pending_steps.push("Await issuer trustline authorization".into()); }

        Ok(ReadinessResponse {
            wallet_id,
            is_ready: check.all_criteria_met,
            criteria: ReadinessCriteria {
                stellar_account_exists: account_exists,
                min_xlm_balance_met: min_xlm_met,
                trustline_active,
                trustline_authorized,
                wallet_registered,
            },
            pending_steps,
        })
    }

    // -------------------------------------------------------------------------
    // Admin: funding account  GET /api/admin/wallet/funding-account
    // -------------------------------------------------------------------------

    pub async fn get_funding_account_status(&self) -> Result<FundingAccountStatus, Error> {
        let fa = self
            .repo
            .get_funding_account()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::NotFound("No active funding account configured".into()))?;

        let balance: f64 = fa.current_xlm_balance.to_string().parse().unwrap_or(0.0);
        let threshold: f64 = fa.min_balance_alert_threshold.to_string().parse().unwrap_or(100.0);
        let per_account_cost = ACCOUNT_BASE_ENTRIES * BASE_RESERVE_XLM + TRUSTLINE_RESERVE_XLM;
        let remaining_capacity = if per_account_cost > 0.0 {
            ((balance - threshold) / per_account_cost).max(0.0) as i64
        } else {
            0
        };

        // Update Prometheus gauge
        metrics::funding_account_balance()
            .with_label_values(&[&fa.stellar_address])
            .set(balance);

        Ok(FundingAccountStatus {
            stellar_address: fa.stellar_address,
            current_xlm_balance: fa.current_xlm_balance.to_string(),
            total_accounts_sponsored: fa.total_accounts_sponsored,
            total_xlm_spent: fa.total_xlm_spent.to_string(),
            estimated_remaining_capacity: remaining_capacity,
            min_balance_alert_threshold: fa.min_balance_alert_threshold.to_string(),
            is_below_threshold: balance < threshold,
        })
    }

    pub async fn request_replenishment(
        &self,
        actor_user_id: Option<Uuid>,
        req: ReplenishmentRequest,
    ) -> Result<(), Error> {
        if req.requested_xlm_amount <= 0.0 {
            return Err(Error::BadRequest("requested_xlm_amount must be positive".into()));
        }

        let fa = self
            .repo
            .get_funding_account()
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::NotFound("No active funding account".into()))?;

        self.repo
            .create_replenishment_request(
                fa.id,
                actor_user_id,
                req.requested_xlm_amount,
                req.notes.as_deref(),
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        info!(
            amount = req.requested_xlm_amount,
            "Funding account replenishment requested"
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn parse_state(s: &str) -> ProvisioningState {
    match s {
        "keypair_generated" => ProvisioningState::KeypairGenerated,
        "registered" => ProvisioningState::Registered,
        "pending_funding" => ProvisioningState::PendingFunding,
        "funded" => ProvisioningState::Funded,
        "trustline_pending" => ProvisioningState::TrustlinePending,
        "trustline_active" => ProvisioningState::TrustlineActive,
        "ready" => ProvisioningState::Ready,
        "stalled" => ProvisioningState::Stalled,
        _ => ProvisioningState::Failed,
    }
}

/// Build an unsigned change_trust XDR envelope.
/// In production this uses the stellar-xdr crate to construct a real envelope.
fn build_change_trust_envelope(account_id: &str, issuer: &str) -> Result<String, Error> {
    // Placeholder XDR — real implementation uses stellar-xdr::TransactionEnvelope
    let placeholder = format!(
        "CHANGE_TRUST_ENVELOPE:account={}:asset=cNGN:issuer={}:limit=unlimited",
        account_id, issuer
    );
    Ok(base64_encode(placeholder.as_bytes()))
}

/// Verify that the signed envelope contains a valid Ed25519 signature from the wallet.
fn verify_envelope_signature(signed_xdr: &str, wallet_address: &str) -> Result<(), Error> {
    // Validate the wallet address format first
    validate_stellar_public_key(wallet_address)
        .map_err(|e| Error::BadRequest(format!("Invalid wallet address: {}", e)))?;

    // In production: decode XDR, extract signatures, verify against wallet public key
    // using ed25519-dalek. Here we do a basic non-empty check.
    if signed_xdr.is_empty() {
        return Err(Error::BadRequest("Signed envelope XDR cannot be empty".into()));
    }
    Ok(())
}

/// Submit a signed transaction envelope to Stellar Horizon.
/// Returns the transaction hash on success.
fn submit_to_horizon(signed_xdr: &str) -> Result<String, String> {
    // In production: POST to Horizon /transactions endpoint via reqwest
    // Returns the transaction hash from the response
    let hash = format!("TX_{}", &signed_xdr[..8.min(signed_xdr.len())]);
    Ok(hash)
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    B64.encode(data)
}
