//! Mint Authorization Service — orchestrates the full cNGN issuance lifecycle.
//!
//! Responsibilities:
//! - Validate reserve verification before creating a request
//! - Build unsigned Stellar transaction XDR and compute tx_hash
//! - Collect and verify Ed25519 signatures from authorized signers
//! - Aggregate signatures into the transaction envelope
//! - Submit to Stellar Horizon with exponential-backoff retry
//! - Expire stale requests via background job
//! - Cancel requests with mandatory justification

use crate::chains::stellar::client::StellarClient;
use crate::mint_authorization::{
    error::MintAuthError,
    metrics,
    models::{
        CancelMintAuthRequest, CreateMintAuthRequest, MintAuthDetail, MintAuthListResponse,
        MintAuthRequest, MintAuthSignature, MintAuthStatus, ListMintAuthQuery,
        SubmitSignatureRequest,
    },
    repository::MintAuthRepository,
};
use crate::multisig::xdr_builder::build_mint_xdr;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::{Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use sqlx::types::BigDecimal;
use std::str::FromStr;
use std::sync::Arc;
use stellar_strkey::ed25519::PublicKey as StrkeyPublicKey;
use stellar_xdr::next::{
    DecoratedSignature, Hash, Limits, ReadXdr, Signature as XdrSignature, SignatureHint,
    TransactionEnvelope, TransactionV1Envelope, VecM, WriteXdr,
};
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Configurable constants (can be overridden via env vars).
const RESERVE_RECENCY_HOURS: i64 = 24;
const AUTH_EXPIRY_HOURS: i64 = 24;
const MAX_RETRY_ATTEMPTS: i16 = 5;

pub struct MintAuthService {
    repo: Arc<MintAuthRepository>,
    stellar: Arc<StellarClient>,
    issuer_address: String,
}

impl MintAuthService {
    pub fn new(
        repo: Arc<MintAuthRepository>,
        stellar: Arc<StellarClient>,
        issuer_address: String,
    ) -> Self {
        Self { repo, stellar, issuer_address }
    }

    pub fn from_env(repo: Arc<MintAuthRepository>, stellar: Arc<StellarClient>) -> Self {
        let issuer_address =
            std::env::var("STELLAR_ISSUER_ADDRESS").unwrap_or_default();
        Self::new(repo, stellar, issuer_address)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Create authorization request (Task 4)
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn create(
        &self,
        req: CreateMintAuthRequest,
        requester_id: Uuid,
        requester_key: &str,
    ) -> Result<MintAuthRequest, MintAuthError> {
        // 1. Validate reserve verification recency and sufficiency
        let (fiat_reserves, in_transit, verified_at) = self
            .repo
            .get_reserve_verification(req.reserve_verification_id)
            .await?
            .ok_or(MintAuthError::ReserveVerificationNotFound(
                req.reserve_verification_id,
            ))?;

        let age_hours = (Utc::now() - verified_at).num_minutes() as f64 / 60.0;
        let max_hours = std::env::var("MINT_AUTH_RESERVE_RECENCY_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(RESERVE_RECENCY_HOURS);

        if age_hours > max_hours as f64 {
            return Err(MintAuthError::ReserveVerificationStale {
                max_hours,
                actual_hours: age_hours,
            });
        }

        // 2. Validate amount against available reserve
        let amount_cngn = BigDecimal::from_str(&req.amount_cngn)
            .map_err(|_| MintAuthError::Config(format!("invalid amount: {}", req.amount_cngn)))?;

        let available = fiat_reserves + in_transit;
        if amount_cngn > available {
            return Err(MintAuthError::ExceedsReserveBalance {
                requested: req.amount_cngn.clone(),
                available: available.to_string(),
            });
        }

        // 3. Fetch issuer sequence number and build unsigned XDR
        let account = self
            .stellar
            .get_account(&self.issuer_address)
            .await
            .map_err(|e| MintAuthError::XdrBuild(e.to_string()))?;

        let amount_stroops = bigdecimal_to_stroops(&amount_cngn)?;
        let unsigned_xdr = build_mint_xdr(
            &self.issuer_address,
            &req.destination_account,
            amount_stroops,
            account.sequence,
        )
        .map_err(|e| MintAuthError::XdrBuild(e.to_string()))?;

        // 4. Compute tx_hash — the SHA-256 hash all signers must sign
        let tx_hash = compute_tx_hash(&unsigned_xdr, self.stellar.network().network_passphrase())
            .map_err(|e| MintAuthError::XdrBuild(e))?;

        // 5. Load required threshold
        let required_signatures = self.repo.get_required_threshold().await?;

        // 6. Persist
        let expiry_hours = std::env::var("MINT_AUTH_EXPIRY_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(AUTH_EXPIRY_HOURS);
        let expires_at = Utc::now() + Duration::hours(expiry_hours);

        let auth_req = self
            .repo
            .create_request(
                &amount_cngn,
                &req.destination_account,
                requester_id,
                requester_key,
                &req.justification,
                req.reserve_verification_id,
                required_signatures,
                &unsigned_xdr,
                &tx_hash,
                expires_at,
            )
            .await?;

        metrics::inc_requests_created();
        metrics::set_pending_count(self.repo.count_pending().await.unwrap_or(0) as f64);

        info!(
            auth_id = %auth_req.id,
            amount_cngn = %amount_cngn,
            destination = %req.destination_account,
            required_signatures,
            expires_at = %expires_at,
            "Mint authorization request created"
        );

        // 7. Notify signers (best-effort)
        self.notify_signers_new_request(&auth_req).await;

        Ok(auth_req)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Submit signature (Task 5)
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn submit_signature(
        &self,
        auth_id: Uuid,
        req: SubmitSignatureRequest,
        ip_address: Option<std::net::IpAddr>,
    ) -> Result<MintAuthDetail, MintAuthError> {
        let auth_req = self
            .repo
            .get_by_id(auth_id)
            .await?
            .ok_or(MintAuthError::NotFound(auth_id))?;

        // Guard: terminal or wrong state
        if auth_req.status.is_terminal() {
            return Err(MintAuthError::TerminalState(
                auth_id,
                auth_req.status.to_string(),
            ));
        }
        if auth_req.status != MintAuthStatus::PendingSignatures {
            return Err(MintAuthError::TerminalState(
                auth_id,
                auth_req.status.to_string(),
            ));
        }

        // Guard: expired
        if Utc::now() > auth_req.expires_at {
            self.repo
                .update_status(auth_id, MintAuthStatus::Expired, None, None, None)
                .await?;
            return Err(MintAuthError::Expired(auth_id));
        }

        // Verify signer is active
        let signer_id = self
            .repo
            .find_active_signer_by_key(&req.signer_key)
            .await?
            .ok_or_else(|| MintAuthError::UnauthorizedSigner(req.signer_key.clone()))?;

        // Duplicate check
        if self.repo.signature_exists(auth_id, signer_id).await? {
            return Err(MintAuthError::DuplicateSignature(
                req.signer_key.clone(),
                auth_id,
            ));
        }

        // Verify Ed25519 signature over tx_hash
        let tx_hash = auth_req
            .tx_hash
            .as_deref()
            .ok_or_else(|| MintAuthError::Config("tx_hash missing on request".into()))?;

        verify_ed25519_signature(&req.signer_key, tx_hash, &req.signature)?;

        // Persist signature (also atomically increments collected_signatures and
        // transitions to threshold_met if threshold reached)
        let sig = self
            .repo
            .add_signature(auth_id, signer_id, &req.signer_key, &req.signature, ip_address)
            .await?;

        metrics::inc_signatures_collected();

        // Re-fetch to get updated status
        let updated = self
            .repo
            .get_by_id(auth_id)
            .await?
            .ok_or(MintAuthError::NotFound(auth_id))?;

        let threshold_just_met = updated.status == MintAuthStatus::ThresholdMet
            && auth_req.status == MintAuthStatus::PendingSignatures;

        if threshold_just_met {
            metrics::inc_thresholds_met();
            info!(auth_id = %auth_id, "Mint authorization threshold met — triggering submission");
            // Trigger submission asynchronously
            let service = self.clone_for_spawn();
            tokio::spawn(async move {
                if let Err(e) = service.submit_to_stellar(auth_id).await {
                    error!(auth_id = %auth_id, error = %e, "Stellar submission failed");
                }
            });
        }

        let signatures = self.repo.list_signatures(auth_id).await?;
        let collected = signatures.len();
        let required = updated.required_signatures as usize;

        info!(
            auth_id = %auth_id,
            signer = %req.signer_key,
            collected,
            required,
            "Signature collected"
        );

        Ok(MintAuthDetail {
            request: updated,
            signatures,
            signatures_collected: collected,
            signatures_required: required,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Signature aggregation + Stellar submission (Tasks 6 & 7)
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn submit_to_stellar(&self, auth_id: Uuid) -> Result<(), MintAuthError> {
        let auth_req = self
            .repo
            .get_by_id(auth_id)
            .await?
            .ok_or(MintAuthError::NotFound(auth_id))?;

        if auth_req.status != MintAuthStatus::ThresholdMet {
            return Err(MintAuthError::NotReadyForSubmission(auth_id));
        }

        // Aggregate signatures into the envelope
        let signatures = self.repo.list_signatures(auth_id).await?;
        let signed_xdr = aggregate_signatures(&auth_req.unsigned_xdr, &signatures)?;

        // Retry loop with exponential backoff
        let max_attempts = std::env::var("MINT_AUTH_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(MAX_RETRY_ATTEMPTS);

        let mut attempt = 0u32;
        loop {
            metrics::inc_submissions_attempted();
            attempt += 1;

            match self.stellar.submit_transaction_xdr(&signed_xdr).await {
                Ok(result) => {
                    let stellar_tx_hash = result
                        .get("hash")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    self.repo
                        .update_status(
                            auth_id,
                            MintAuthStatus::Submitted,
                            Some(&signed_xdr),
                            Some(&stellar_tx_hash),
                            None,
                        )
                        .await?;

                    info!(
                        auth_id = %auth_id,
                        stellar_tx_hash = %stellar_tx_hash,
                        "Mint authorization submitted to Stellar"
                    );

                    // Monitor for confirmation asynchronously
                    let service = self.clone_for_spawn();
                    let hash = stellar_tx_hash.clone();
                    tokio::spawn(async move {
                        service.monitor_confirmation(auth_id, &hash).await;
                    });

                    return Ok(());
                }
                Err(e) => {
                    let is_transient = is_transient_stellar_error(&e.to_string());
                    if !is_transient || attempt >= max_attempts as u32 {
                        let reason = e.to_string();
                        error!(
                            auth_id = %auth_id,
                            attempt,
                            error = %reason,
                            "Mint authorization Stellar submission failed permanently"
                        );
                        self.repo
                            .update_status(
                                auth_id,
                                MintAuthStatus::Failed,
                                None,
                                None,
                                Some(&reason),
                            )
                            .await?;
                        metrics::inc_failures();
                        return Err(MintAuthError::StellarSubmission(reason));
                    }

                    self.repo.increment_retry(auth_id).await?;
                    let backoff = std::time::Duration::from_secs(2u64.pow(attempt));
                    warn!(
                        auth_id = %auth_id,
                        attempt,
                        backoff_secs = backoff.as_secs(),
                        error = %e,
                        "Transient Stellar error — retrying"
                    );
                    sleep(backoff).await;
                }
            }
        }
    }

    /// Poll Horizon until the transaction is confirmed or times out.
    async fn monitor_confirmation(&self, auth_id: Uuid, stellar_tx_hash: &str) {
        let max_polls = 30u32;
        let poll_interval = std::time::Duration::from_secs(10);

        for _ in 0..max_polls {
            sleep(poll_interval).await;
            match self.stellar.get_transaction_details(stellar_tx_hash).await {
                Ok(tx) if tx.successful => {
                    if let Err(e) = self
                        .repo
                        .update_status(
                            auth_id,
                            MintAuthStatus::Confirmed,
                            None,
                            None,
                            None,
                        )
                        .await
                    {
                        error!(auth_id = %auth_id, error = %e, "Failed to mark confirmed");
                    } else {
                        metrics::inc_confirmations_received();
                        info!(
                            auth_id = %auth_id,
                            stellar_tx_hash,
                            "Mint authorization confirmed on Stellar"
                        );
                    }
                    return;
                }
                Ok(_) => {
                    warn!(auth_id = %auth_id, "Transaction found but not successful");
                    return;
                }
                Err(_) => {
                    // Not yet confirmed — keep polling
                }
            }
        }
        warn!(auth_id = %auth_id, "Confirmation polling timed out");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Cancellation (Task 9)
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn cancel(
        &self,
        auth_id: Uuid,
        cancelled_by: Uuid,
        req: CancelMintAuthRequest,
    ) -> Result<MintAuthRequest, MintAuthError> {
        let cancelled = self
            .repo
            .cancel(auth_id, cancelled_by, &req.justification)
            .await?;

        metrics::inc_cancellations();
        metrics::set_pending_count(self.repo.count_pending().await.unwrap_or(0) as f64);

        info!(
            auth_id = %auth_id,
            cancelled_by = %cancelled_by,
            reason = %req.justification,
            "Mint authorization cancelled"
        );

        Ok(cancelled)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Read operations
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn get(&self, auth_id: Uuid) -> Result<MintAuthDetail, MintAuthError> {
        let request = self
            .repo
            .get_by_id(auth_id)
            .await?
            .ok_or(MintAuthError::NotFound(auth_id))?;
        let signatures = self.repo.list_signatures(auth_id).await?;
        let collected = signatures.len();
        let required = request.required_signatures as usize;
        Ok(MintAuthDetail {
            request,
            signatures,
            signatures_collected: collected,
            signatures_required: required,
        })
    }

    pub async fn list(&self, query: ListMintAuthQuery) -> Result<MintAuthListResponse, MintAuthError> {
        let (items, total) = self
            .repo
            .list(query.status, query.limit(), query.offset())
            .await?;
        Ok(MintAuthListResponse {
            total,
            limit: query.limit(),
            offset: query.offset(),
            items,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Expiry worker (Task 8)
    // ─────────────────────────────────────────────────────────────────────────

    /// Run once: expire all overdue pending requests.
    pub async fn expire_stale_requests(&self) -> Result<usize, MintAuthError> {
        let expired = self.repo.find_expired().await?;
        let count = expired.len();

        for req in &expired {
            self.repo
                .update_status(req.id, MintAuthStatus::Expired, None, None, None)
                .await?;
            metrics::inc_expirations();
            info!(auth_id = %req.id, "Mint authorization expired");
        }

        if count > 0 {
            metrics::set_pending_count(self.repo.count_pending().await.unwrap_or(0) as f64);
        }

        Ok(count)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Notify all active signers of a new pending request (best-effort).
    async fn notify_signers_new_request(&self, req: &MintAuthRequest) {
        match self.repo.list_active_signer_emails().await {
            Ok(emails) => {
                info!(
                    auth_id = %req.id,
                    signer_count = emails.len(),
                    "Notifying signers of new mint authorization request"
                );
                // Notification delivery is handled by the caller's notification infrastructure.
                // Log the event so downstream systems can pick it up.
            }
            Err(e) => {
                warn!(auth_id = %req.id, error = %e, "Failed to fetch signer emails for notification");
            }
        }
    }

    /// Cheap clone for spawning tasks — only clones the Arcs.
    fn clone_for_spawn(&self) -> Self {
        Self {
            repo: Arc::clone(&self.repo),
            stellar: Arc::clone(&self.stellar),
            issuer_address: self.issuer_address.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure functions (testable without DB/Stellar)
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the Stellar transaction hash (network-qualified SHA-256) from unsigned XDR.
pub fn compute_tx_hash(unsigned_xdr: &str, network_passphrase: &str) -> Result<String, String> {
    let envelope = TransactionEnvelope::from_xdr_base64(unsigned_xdr, Limits::none())
        .map_err(|e| format!("XDR decode error: {e}"))?;

    let tx = match &envelope {
        TransactionEnvelope::Tx(v1) => &v1.tx,
        _ => return Err("unsupported envelope type".into()),
    };

    let network_id: [u8; 32] = Sha256::digest(network_passphrase.as_bytes()).into();
    let hash = tx
        .hash(Hash(network_id))
        .map_err(|e| format!("hash error: {e}"))?;

    Ok(hex::encode(hash.0))
}

/// Verify an Ed25519 signature over a hex-encoded tx_hash.
pub fn verify_ed25519_signature(
    stellar_public_key: &str,
    tx_hash_hex: &str,
    signature_b64: &str,
) -> Result<(), MintAuthError> {
    // Decode Stellar public key (G…) → raw 32-byte Ed25519 key
    let strkey = StrkeyPublicKey::from_string(stellar_public_key).map_err(|_| {
        MintAuthError::InvalidSignature(
            stellar_public_key.to_string(),
            "invalid Stellar public key".into(),
        )
    })?;
    let verifying_key = VerifyingKey::from_bytes(&strkey.0).map_err(|e| {
        MintAuthError::InvalidSignature(stellar_public_key.to_string(), e.to_string())
    })?;

    // Decode signature
    let sig_bytes = B64.decode(signature_b64).map_err(|_| {
        MintAuthError::InvalidSignature(
            stellar_public_key.to_string(),
            "invalid base64 signature".into(),
        )
    })?;
    let signature = Signature::from_slice(&sig_bytes).map_err(|e| {
        MintAuthError::InvalidSignature(stellar_public_key.to_string(), e.to_string())
    })?;

    // Decode tx_hash
    let hash_bytes = hex::decode(tx_hash_hex).map_err(|_| {
        MintAuthError::InvalidSignature(
            stellar_public_key.to_string(),
            "invalid tx_hash hex".into(),
        )
    })?;

    verifying_key.verify(&hash_bytes, &signature).map_err(|e| {
        MintAuthError::InvalidSignature(stellar_public_key.to_string(), e.to_string())
    })
}

/// Aggregate collected signatures into the unsigned XDR envelope.
pub fn aggregate_signatures(
    unsigned_xdr: &str,
    signatures: &[MintAuthSignature],
) -> Result<String, MintAuthError> {
    let envelope = TransactionEnvelope::from_xdr_base64(unsigned_xdr, Limits::none())
        .map_err(|e| MintAuthError::XdrBuild(e.to_string()))?;

    let tx = match envelope {
        TransactionEnvelope::Tx(v1) => v1.tx,
        _ => return Err(MintAuthError::XdrBuild("unsupported envelope type".into())),
    };

    let mut decorated: Vec<DecoratedSignature> = Vec::with_capacity(signatures.len());

    for sig in signatures {
        // Decode the signer's public key to derive the 4-byte hint
        let strkey = StrkeyPublicKey::from_string(&sig.signer_key)
            .map_err(|_| MintAuthError::XdrBuild(format!("invalid key: {}", sig.signer_key)))?;
        let hint_bytes: [u8; 4] = strkey.0[28..32].try_into().unwrap();
        let hint = SignatureHint(hint_bytes);

        // Decode the raw 64-byte signature
        let sig_bytes = B64
            .decode(&sig.signature)
            .map_err(|e| MintAuthError::XdrBuild(format!("base64 decode: {e}")))?;
        let xdr_sig = XdrSignature::try_from(sig_bytes)
            .map_err(|e| MintAuthError::XdrBuild(format!("signature bytes: {e}")))?;

        decorated.push(DecoratedSignature { hint, signature: xdr_sig });
    }

    let sigs_vec: VecM<DecoratedSignature, 20> = decorated
        .try_into()
        .map_err(|_| MintAuthError::XdrBuild("too many signatures".into()))?;

    let signed_env = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: sigs_vec,
    });

    signed_env
        .to_xdr_base64(Limits::none())
        .map_err(|e| MintAuthError::XdrBuild(e.to_string()))
}

/// Convert BigDecimal cNGN amount to Stellar stroops (1 cNGN = 10_000_000 stroops).
fn bigdecimal_to_stroops(amount: &BigDecimal) -> Result<i64, MintAuthError> {
    use std::str::FromStr;
    let stroops = amount * BigDecimal::from_str("10000000").unwrap();
    stroops
        .to_string()
        .split('.')
        .next()
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| MintAuthError::Config(format!("cannot convert {amount} to stroops")))
}

/// Heuristic: treat rate-limit and timeout errors as transient.
fn is_transient_stellar_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("timeout")
        || lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("503")
        || lower.contains("502")
        || lower.contains("connection")
}
