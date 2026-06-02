//! Proof-of-Reserves (PoR) Worker — Issue #297
//!
//! Runs every 60 minutes and:
//!   1. Fetches the current cNGN circulating supply from Stellar Horizon.
//!   2. Aggregates settled NGN balances from custodian bank APIs (read-only).
//!   3. Calculates the collateralization ratio:
//!        ratio = (total_bank_assets / total_on_chain_supply) × 100
//!   4. Persists a signed PoR snapshot to `por_snapshots`.
//!   5. Raises an investigation alert to the Audit Trail if the ratio deviates
//!      more than 0.05% from 100.00% (i.e. ratio < 99.95%).
//!   6. Raises an under-collateralization alert if ratio < 100.01%.

use crate::audit::models::{AuditActorType, AuditEventCategory, AuditOutcome, PendingAuditEntry};
use crate::audit::writer::AuditWriter;
use crate::chains::stellar::client::StellarClient;
use crate::security::{AnomalyDetectionService, AnomalyType, TenantBalance, MerkleTree};
use crate::metrics::por::{
    merkle_tree_construction_duration_seconds, proof_anchoring_failures_total,
    reserve_backing_ratio, total_fiat_reserves_held,
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use ed25519_dalek::Signer;
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{error, info, instrument, warn};
use stellar_sdk::types::{Asset, Memo, PublicKey};
use stellar_sdk::{KeyPair, Network, Server, TransactionBuilder};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Ratio must be >= 100.01% to be considered fully collateralised (issue spec).
const UNDER_COLLATERAL_THRESHOLD: f64 = 100.01;
/// Discrepancies > 0.05% from 100.00% trigger an investigation alert.
const DISCREPANCY_ALERT_THRESHOLD_PCT: f64 = 0.05;
/// Default PoR refresh interval: 60 minutes.
const DEFAULT_INTERVAL_SECS: u64 = 60 * 60;

// ── Bank credential (read-only) ───────────────────────────────────────────────

/// A single custodian bank configured via environment variables.
/// Only "settled balance" (read-only) credentials are stored — the service
/// cannot initiate transfers.
#[derive(Debug, Clone)]
pub struct BankCredential {
    /// Anonymised label shown in the public PoR response, e.g. "Reserve Vault A".
    pub label: String,
    /// Base URL of the bank's balance API.
    pub api_base_url: String,
    /// Read-only API key / bearer token.
    pub api_key: String,
    /// Account identifier to query.
    pub account_id: String,
}

impl BankCredential {
    /// Load all configured bank credentials from environment variables.
    ///
    /// Convention:
    ///   RESERVE_BANK_1_LABEL, RESERVE_BANK_1_API_URL,
    ///   RESERVE_BANK_1_API_KEY, RESERVE_BANK_1_ACCOUNT_ID
    ///   … up to RESERVE_BANK_9_*
    pub fn load_from_env() -> Vec<Self> {
        let mut banks = Vec::new();
        for i in 1..=9 {
            let label = std::env::var(format!("RESERVE_BANK_{i}_LABEL")).unwrap_or_default();
            let url = std::env::var(format!("RESERVE_BANK_{i}_API_URL")).unwrap_or_default();
            let key = std::env::var(format!("RESERVE_BANK_{i}_API_KEY")).unwrap_or_default();
            let account = std::env::var(format!("RESERVE_BANK_{i}_ACCOUNT_ID")).unwrap_or_default();

            if label.is_empty() || url.is_empty() || key.is_empty() || account.is_empty() {
                continue;
            }
            banks.push(BankCredential {
                label,
                api_base_url: url,
                api_key: key,
                account_id: account,
            });
        }
        banks
    }
}

// ── Bank balance result ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BankBalance {
    pub label: String,
    pub settled_balance: BigDecimal,
    pub currency: String,
    /// Timestamp from the bank API confirming the balance (Proof of Solvency).
    pub balance_as_of: chrono::DateTime<Utc>,
    pub signature: String,
    pub signing_key: String,
}

// ── Worker ────────────────────────────────────────────────────────────────────

pub struct ProofOfReservesWorker {
    pool: PgPool,
    stellar_client: StellarClient,
    banks: Vec<BankCredential>,
    signing_key: Arc<ed25519_dalek::SigningKey>,
    audit_writer: Option<Arc<AuditWriter>>,
    cngn_asset_code: String,
    cngn_issuer: String,
    interval: Duration,
    http: reqwest::Client,
    anomaly_service: Option<Arc<AnomalyDetectionService>>,
}

impl ProofOfReservesWorker {
    pub fn new(
        pool: PgPool,
        stellar_client: StellarClient,
        signing_key: Arc<ed25519_dalek::SigningKey>,
        audit_writer: Option<Arc<AuditWriter>>,
        cngn_issuer: String,
        anomaly_service: Option<Arc<AnomalyDetectionService>>,
    ) -> Self {
        let interval_secs = std::env::var("POR_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_INTERVAL_SECS);

        Self {
            pool,
            stellar_client,
            banks: BankCredential::load_from_env(),
            signing_key,
            audit_writer,
            cngn_asset_code: "cNGN".to_string(),
            cngn_issuer,
            interval: Duration::from_secs(interval_secs),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            anomaly_service,
        }
    }

    pub async fn run(self, mut shutdown_rx: watch::Receiver<bool>) {
        info!(
            interval_mins = self.interval.as_secs() / 60,
            asset = %format!("{}:{}", self.cngn_asset_code, self.cngn_issuer),
            banks = self.banks.len(),
            "Proof-of-Reserves worker started"
        );

        let mut ticker = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Proof-of-Reserves worker stopping");
                        break;
                    }
                }
                _ = ticker.tick() => {
                    if let Err(e) = self.run_cycle().await {
                        error!(error = %e, "PoR cycle failed");
                    }
                }
            }
        }
    }

    #[instrument(skip(self), name = "por_cycle")]
    async fn run_cycle(&self) -> anyhow::Result<()> {
        info!("Starting Proof-of-Reserves cycle");

        // 1. On-chain supply
        let circulating_supply = match self.fetch_on_chain_supply().await {
            Ok(supply) => supply,
            Err(e) => {
                error!(error = %e, "Failed to fetch circulating supply");
                proof_anchoring_failures_total().with_label_values(&[]).inc();
                return Err(e);
            }
        };

        // 2. Fetch AMM reserves
        let amm_reserves = match self.fetch_amm_reserves().await {
            Ok(reserves) => reserves,
            Err(e) => {
                warn!(error = %e, "Failed to fetch AMM reserves, defaulting to 0");
                BigDecimal::from(0)
            }
        };

        let total_on_chain_supply = circulating_supply + amm_reserves;

        // 3. Bank balances
        let bank_balances = self.fetch_bank_balances().await;

        // 4. Aggregate bank assets
        let total_bank_assets: BigDecimal = bank_balances
            .iter()
            .fold(BigDecimal::from(0), |acc, b| acc + &b.settled_balance);

        // Custodian solvency timestamp: earliest balance_as_of across all banks
        let custodian_solvency_ts = bank_balances
            .iter()
            .map(|b| b.balance_as_of)
            .min()
            .unwrap_or_else(Utc::now);

        // 5. Collateralization ratio (as percentage, e.g. 100.15%)
        let ratio_pct = if total_on_chain_supply == BigDecimal::from(0) {
            BigDecimal::from(100)
        } else {
            (&total_bank_assets / &total_on_chain_supply.clone()) * BigDecimal::from(100)
        };
        let ratio_f64: f64 = ratio_pct.to_string().parse().unwrap_or(0.0);
        let is_fully_collateralized = ratio_f64 >= UNDER_COLLATERAL_THRESHOLD;

        // Reserve backing ratio (as fraction for audit_snapshots, e.g. 1.0015)
        let reserve_backing_ratio_val = if total_on_chain_supply == BigDecimal::from(0) {
            BigDecimal::from(1)
        } else {
            &total_bank_assets / &total_on_chain_supply.clone()
        };

        // Update Prometheus metrics for backing ratio and total reserves
        let total_bank_assets_f64 = total_bank_assets.to_string().parse::<f64>().unwrap_or(0.0);
        reserve_backing_ratio().with_label_values(&[]).set(ratio_f64);
        total_fiat_reserves_held().with_label_values(&[]).set(total_bank_assets_f64);

        // 6. Build and sign canonical payload for legacy compatibility
        let canonical = format!(
            r#"{{"collateralization_ratio":"{ratio_pct}","custodian_solvency_ts":"{custodian_ts}","total_bank_assets":"{bank}","total_on_chain_supply":"{supply}"}}"#,
            ratio_pct = ratio_pct,
            custodian_ts = custodian_solvency_ts.to_rfc3339(),
            bank = total_bank_assets,
            supply = total_on_chain_supply,
        );
        let sig_bytes = self.signing_key.sign(canonical.as_bytes());
        let signature = hex::encode(sig_bytes.to_bytes());
        let signing_key_hex = hex::encode(self.signing_key.verifying_key().to_bytes());

        // 7. Check for reserves parity breach (reserves < supply)
        let is_parity_breached = total_bank_assets < total_on_chain_supply;
        if is_parity_breached {
            error!(
                supply = %total_on_chain_supply,
                reserves = %total_bank_assets,
                "🚨 Proof-of-Reserves parity breach detected! Custodian reserves are lower than outstanding supply!"
            );

            // Trip the circuit breaker
            if let Some(anomaly_service) = &self.anomaly_service {
                let bank_reserves_u64 = total_bank_assets_f64 as u64;
                let on_chain_supply_u64 = total_on_chain_supply.to_string().parse::<f64>().unwrap_or(0.0) as u64;
                let delta = on_chain_supply_u64.saturating_sub(bank_reserves_u64);
                let delta_percentage = if on_chain_supply_u64 > 0 {
                    (delta as f64) / (on_chain_supply_u64 as f64)
                } else {
                    0.0
                };

                let anomaly = AnomalyType::NegativeDelta {
                    bank_reserves: bank_reserves_u64,
                    on_chain_supply: on_chain_supply_u64,
                    delta_percentage,
                };
                if let Err(e) = anomaly_service.trigger_circuit_breaker(anomaly).await {
                    error!(error = %e, "Failed to trigger circuit breaker");
                }
            }

            // Persist the audit snapshot and banking logs without anchoring (since we aborted anchoring)
            let snapshot_id = self
                .persist_snapshot(
                    &total_on_chain_supply,
                    &total_bank_assets,
                    &ratio_pct,
                    is_fully_collateralized,
                    custodian_solvency_ts,
                    &signature,
                    &signing_key_hex,
                    &bank_balances,
                    &reserve_backing_ratio_val,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;

            self.raise_discrepancy_alert(
                snapshot_id,
                &ratio_pct,
                &total_on_chain_supply,
                &total_bank_assets,
                "CRITICAL",
            )
            .await;

            proof_anchoring_failures_total().with_label_values(&[]).inc();
            return Ok(());
        }

        // 8. No breach -> build Merkle Tree from active tenant balances
        let tenant_balances = match self.fetch_tenant_balances().await {
            Ok(balances) => balances,
            Err(e) => {
                error!(error = %e, "Failed to fetch tenant balances from database");
                proof_anchoring_failures_total().with_label_values(&[]).inc();
                return Err(e);
            }
        };

        let start_merkle = std::time::Instant::now();
        let tree = MerkleTree::build(&tenant_balances);
        let duration_merkle = start_merkle.elapsed();
        merkle_tree_construction_duration_seconds()
            .with_label_values(&[])
            .observe(duration_merkle.as_secs_f64());

        let merkle_root_hex = tree.root_hex();
        let depth = tree.tree_depth();

        // 9. Anchor Merkle Root on Stellar
        let (tx_hash, ledger) = match self.submit_to_stellar(&merkle_root_hex).await {
            Ok(res) => res,
            Err(e) => {
                error!(error = %e, "Failed to anchor Merkle root onto Stellar");
                proof_anchoring_failures_total().with_label_values(&[]).inc();
                (None, None)
            }
        };

        // 10. Persist complete snapshot
        let snapshot_id = self
            .persist_snapshot(
                &total_on_chain_supply,
                &total_bank_assets,
                &ratio_pct,
                is_fully_collateralized,
                custodian_solvency_ts,
                &signature,
                &signing_key_hex,
                &bank_balances,
                &reserve_backing_ratio_val,
                Some(&merkle_root_hex),
                ledger,
                tx_hash.as_deref(),
                Some(depth),
            )
            .await?;

        info!(
            snapshot_id = %snapshot_id,
            supply = %total_on_chain_supply,
            bank_assets = %total_bank_assets,
            ratio_pct = ratio_f64,
            fully_collateralized = is_fully_collateralized,
            merkle_root = %merkle_root_hex,
            stellar_tx = ?tx_hash,
            "PoR snapshot recorded and Merkle Root anchored successfully"
        );

        // 11. Under-collateralization alert (ratio < 100.01%)
        if !is_fully_collateralized {
            warn!(
                ratio_pct = ratio_f64,
                threshold = UNDER_COLLATERAL_THRESHOLD,
                "⚠️  cNGN is UNDER-COLLATERALIZED — ratio below 100.01%"
            );
            self.raise_discrepancy_alert(
                snapshot_id,
                &ratio_pct,
                &total_on_chain_supply,
                &total_bank_assets,
                "CRITICAL",
            )
            .await;
        }

        // 12. Discrepancy investigation alert (deviation > 0.05% from 100.00%)
        let deviation = (ratio_f64 - 100.0_f64).abs();
        if deviation > DISCREPANCY_ALERT_THRESHOLD_PCT {
            warn!(
                deviation_pct = deviation,
                threshold_pct = DISCREPANCY_ALERT_THRESHOLD_PCT,
                "🔍 PoR discrepancy exceeds 0.05% — raising investigation alert"
            );
            self.raise_discrepancy_alert(
                snapshot_id,
                &ratio_pct,
                &total_on_chain_supply,
                &total_bank_assets,
                "INVESTIGATION",
            )
            .await;
        }

        Ok(())
    }

    // ── On-chain supply ───────────────────────────────────────────────────────

    async fn fetch_on_chain_supply(&self) -> anyhow::Result<BigDecimal> {
        let stats = self
            .stellar_client
            .get_asset_stats(&self.cngn_asset_code, &self.cngn_issuer)
            .await?;

        let amount_str = stats.get("amount").and_then(|v| v.as_str()).unwrap_or("0");

        Ok(BigDecimal::from_str(amount_str).unwrap_or_else(|_| BigDecimal::from(0)))
    }

    // ── AMM Reserves ──────────────────────────────────────────────────────────

    async fn fetch_amm_reserves(&self) -> anyhow::Result<BigDecimal> {
        let pool_id = match std::env::var("CNGN_LIQUIDITY_POOL_ID") {
            Ok(id) if !id.is_empty() => id,
            _ => return Ok(BigDecimal::from(0)),
        };

        let url = format!("{}/liquidity_pools/{}", self.stellar_client.config().horizon_url(), pool_id);
        
        #[derive(Debug, serde::Deserialize)]
        struct PoolReserve {
            asset: String,
            amount: String,
        }

        #[derive(Debug, serde::Deserialize)]
        struct LiquidityPoolResponse {
            reserves: Vec<PoolReserve>,
        }

        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            warn!(status = ?resp.status(), "Failed to fetch liquidity pool reserves from Horizon");
            return Ok(BigDecimal::from(0));
        }

        let body: LiquidityPoolResponse = resp.json().await?;
        let cngn_asset = format!("{}:{}", self.cngn_asset_code, self.cngn_issuer);

        for reserve in body.reserves {
            if reserve.asset == cngn_asset {
                let amount = BigDecimal::from_str(&reserve.amount).unwrap_or_else(|_| BigDecimal::from(0));
                return Ok(amount);
            }
        }

        Ok(BigDecimal::from(0))
    }

    // ── Tenant Balances ───────────────────────────────────────────────────────

    async fn fetch_tenant_balances(&self) -> anyhow::Result<Vec<TenantBalance>> {
        let rows = sqlx::query!(
            r#"
            SELECT wallet_address, cngn_balance
            FROM wallets
            WHERE cngn_balance > 0
            ORDER BY wallet_address
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let balances = rows
            .into_iter()
            .map(|r| TenantBalance {
                tenant_id: r.wallet_address,
                balance: r.cngn_balance,
            })
            .collect();

        Ok(balances)
    }

    // ── Bank balances ─────────────────────────────────────────────────────────

    /// Fetch settled balances from all configured custodian banks.
    /// Uses read-only credentials — cannot initiate transfers.
    async fn fetch_bank_balances(&self) -> Vec<BankBalance> {
        let mut results = Vec::new();

        for bank in &self.banks {
            match self.fetch_single_bank_balance(bank).await {
                Ok(balance) => results.push(balance),
                Err(e) => {
                    error!(
                        bank = %bank.label,
                        error = %e,
                        "Failed to fetch bank balance — treating as 0"
                    );
                    results.push(BankBalance {
                        label: bank.label.clone(),
                        settled_balance: BigDecimal::from(0),
                        currency: "NGN".to_string(),
                        balance_as_of: Utc::now(),
                        signature: "error-signature".to_string(),
                        signing_key: "error-signing-key".to_string(),
                    });
                }
            }
        }

        if results.is_empty() {
            warn!("No custodian bank credentials configured (RESERVE_BANK_1_* env vars). PoR bank total will be 0.");
        }

        results
    }

    /// Fetch a single bank's settled balance via its read-only API.
    async fn fetch_single_bank_balance(
        &self,
        bank: &BankCredential,
    ) -> anyhow::Result<BankBalance> {
        let url = format!(
            "{}/accounts/{}/settled-balance",
            bank.api_base_url.trim_end_matches('/'),
            bank.account_id
        );

        let resp = self
            .http
            .get(&url)
            .bearer_auth(&bank.api_key)
            .header("X-Read-Only", "true")
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let balance_str = resp
            .get("settled_balance")
            .and_then(|v| v.as_str())
            .unwrap_or("0");
        let currency = resp
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("NGN")
            .to_string();
        let balance_as_of_str = resp
            .get("balance_as_of")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let balance_as_of = chrono::DateTime::parse_from_rfc3339(balance_as_of_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let signature = resp
            .get("signature")
            .and_then(|v| v.as_str())
            .unwrap_or("stub-signature")
            .to_string();

        let signing_key = resp
            .get("signing_key")
            .and_then(|v| v.as_str())
            .unwrap_or("stub-signing-key")
            .to_string();

        Ok(BankBalance {
            label: bank.label.clone(),
            settled_balance: BigDecimal::from_str(balance_str)
                .unwrap_or_else(|_| BigDecimal::from(0)),
            currency,
            balance_as_of,
            signature,
            signing_key,
        })
    }

    // ── Stellar Anchoring ─────────────────────────────────────────────────────

    async fn submit_to_stellar(&self, merkle_root_hex: &str) -> anyhow::Result<(Option<String>, Option<i64>)> {
        let source_secret = match std::env::var("POR_ANCHOR_SECRET") {
            Ok(secret) if !secret.is_empty() => secret,
            _ => {
                warn!("POR_ANCHOR_SECRET is not set. Skipping Stellar anchoring.");
                return Ok((None, None));
            }
        };

        let destination_account = std::env::var("POR_ANCHOR_DESTINATION").ok();

        let source_keypair = KeyPair::from_secret_seed(&source_secret)
            .map_err(|e| anyhow::anyhow!("Invalid secret key: {}", e))?;

        let horizon_url = self.stellar_client.config().horizon_url();
        let server = Server::new(horizon_url)
            .map_err(|e| anyhow::anyhow!("Failed to create Stellar server: {}", e))?;

        let source_account = server
            .load_account(&source_keypair.public_key())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to load source account: {}", e))?;

        let destination = if let Some(ref dest) = destination_account {
            PublicKey::from_account_id(dest)
                .map_err(|e| anyhow::anyhow!("Invalid destination account ID: {}", e))?
        } else {
            source_keypair.public_key()
        };

        let root_bytes = hex::decode(merkle_root_hex)
            .map_err(|e| anyhow::anyhow!("Failed to decode Merkle root hex: {}", e))?;
        if root_bytes.len() != 32 {
            return Err(anyhow::anyhow!("Merkle root must be exactly 32 bytes"));
        }
        let memo = Memo::hash(&root_bytes);

        let passphrase = self.stellar_client.config().network.network_passphrase();
        let mut tx_builder = TransactionBuilder::new(
            source_account,
            Network::from_passphrase(passphrase),
        )
        .base_fee(100)
        .memo(memo);

        tx_builder = tx_builder.add_operation(
            stellar_sdk::operations::Payment::new(
                destination,
                Asset::native(),
                "0.0000001",
            )
            .build(),
        );

        let transaction = tx_builder.build()?;
        let signed_tx = transaction.sign(&source_keypair)?;
        let response = server.submit_transaction(&signed_tx).await?;

        Ok((Some(response.hash), Some(response.ledger as i64)))
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    async fn persist_snapshot(
        &self,
        total_on_chain_supply: &BigDecimal,
        total_bank_assets: &BigDecimal,
        collateralization_ratio: &BigDecimal,
        is_fully_collateralized: bool,
        custodian_solvency_ts: chrono::DateTime<Utc>,
        signature: &str,
        signing_key: &str,
        bank_balances: &[BankBalance],
        reserve_backing_ratio: &BigDecimal,
        merkle_root_hash: Option<&str>,
        block_confirmation_height: Option<i64>,
        anchoring_tx_signature: Option<&str>,
        tree_depth: Option<usize>,
    ) -> anyhow::Result<uuid::Uuid> {
        let mut tx = self.pool.begin().await?;

        // A. Insert into legacy por_snapshots table
        let snapshot_id = sqlx::query_scalar::<_, uuid::Uuid>(
            r#"
            INSERT INTO por_snapshots
                (total_on_chain_supply, total_bank_assets, collateralization_ratio,
                 is_fully_collateralized, custodian_solvency_ts, signature, signing_key)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(total_on_chain_supply)
        .bind(total_bank_assets)
        .bind(collateralization_ratio)
        .bind(is_fully_collateralized)
        .bind(custodian_solvency_ts)
        .bind(signature)
        .bind(signing_key)
        .fetch_one(&mut *tx)
        .await?;

        for bank in bank_balances {
            sqlx::query(
                r#"
                INSERT INTO por_bank_balances
                    (snapshot_id, bank_label, settled_balance, currency, balance_as_of)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(snapshot_id)
            .bind(&bank.label)
            .bind(&bank.settled_balance)
            .bind(&bank.currency)
            .bind(bank.balance_as_of)
            .execute(&mut *tx)
            .await?;
        }

        // B. Insert into new cryptographic audit trail tables
        let audit_snapshot_id = sqlx::query_scalar::<_, uuid::Uuid>(
            r#"
            INSERT INTO audit_snapshots
                (total_on_chain_supply, total_fiat_balances, reserve_backing_ratio, recorded_at, created_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            RETURNING id
            "#,
        )
        .bind(total_on_chain_supply)
        .bind(total_bank_assets)
        .bind(reserve_backing_ratio)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(root_hash) = merkle_root_hash {
            sqlx::query(
                r#"
                INSERT INTO merkle_proof_registry
                    (snapshot_id, merkle_root_hash, block_confirmation_height, anchoring_tx_signature, tree_depth, recorded_at, created_at)
                VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
                "#,
            )
            .bind(audit_snapshot_id)
            .bind(root_hash)
            .bind(block_confirmation_height)
            .bind(anchoring_tx_signature)
            .bind(tree_depth.unwrap_or(0) as i32)
            .execute(&mut *tx)
            .await?;
        }

        for bank in bank_balances {
            sqlx::query(
                r#"
                INSERT INTO fiat_bank_balances_historical
                    (bank_label, settled_balance, currency, signature, signing_key, balance_as_of, recorded_at)
                VALUES ($1, $2, $3, $4, $5, $6, NOW())
                "#,
            )
            .bind(&bank.label)
            .bind(&bank.settled_balance)
            .bind(&bank.currency)
            .bind(&bank.signature)
            .bind(&bank.signing_key)
            .bind(bank.balance_as_of)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(snapshot_id)
    }

    // ── Discrepancy alert ─────────────────────────────────────────────────────

    async fn raise_discrepancy_alert(
        &self,
        snapshot_id: uuid::Uuid,
        ratio: &BigDecimal,
        supply: &BigDecimal,
        bank_assets: &BigDecimal,
        alert_level: &str,
    ) {
        let shortfall = supply - bank_assets;
        let deviation_pct = (ratio.to_string().parse::<f64>().unwrap_or(0.0) - 100.0_f64).abs();
        let deviation_bd = BigDecimal::from_str(&format!("{deviation_pct:.6}"))
            .unwrap_or_else(|_| BigDecimal::from(0));

        let db_result = sqlx::query(
            r#"
            INSERT INTO por_discrepancy_alerts
                (snapshot_id, collateralization_ratio, total_on_chain_supply,
                 total_bank_assets, shortfall, deviation_pct, alert_level)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(snapshot_id)
        .bind(ratio)
        .bind(supply)
        .bind(bank_assets)
        .bind(&shortfall)
        .bind(&deviation_bd)
        .bind(alert_level)
        .execute(&self.pool)
        .await;

        if let Err(e) = db_result {
            error!(error = %e, "Failed to persist PoR discrepancy alert");
        }

        if let Some(writer) = &self.audit_writer {
            let entry = PendingAuditEntry {
                event_type: "por.discrepancy_alert".to_string(),
                event_category: AuditEventCategory::FinancialTransaction,
                actor_type: AuditActorType::System,
                actor_id: Some("por_worker".to_string()),
                actor_ip: None,
                actor_consumer_type: Some("proof_of_reserves_worker".to_string()),
                session_id: None,
                target_resource_type: Some("por_snapshot".to_string()),
                target_resource_id: Some(snapshot_id.to_string()),
                request_method: "INTERNAL".to_string(),
                request_path: "/internal/por/discrepancy".to_string(),
                request_body_hash: None,
                response_status: 200,
                response_latency_ms: 0,
                outcome: AuditOutcome::Failure,
                failure_reason: Some(format!(
                    "PoR {alert_level}: ratio={ratio:.6}% supply={supply} bank_assets={bank_assets} \
                     shortfall={shortfall} deviation={deviation_pct:.4}%"
                )),
                environment: std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string()),
            };
            writer.write(entry).await;
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_calculation_fully_backed() {
        let supply = BigDecimal::from_str("1000000").unwrap();
        let bank = BigDecimal::from_str("1001000").unwrap();
        let ratio = (&bank / &supply) * BigDecimal::from(100);
        let ratio_f64: f64 = ratio.to_string().parse().unwrap();
        assert!(ratio_f64 >= UNDER_COLLATERAL_THRESHOLD);
    }

    #[test]
    fn ratio_calculation_under_collateralized() {
        let supply = BigDecimal::from_str("1000000").unwrap();
        let bank = BigDecimal::from_str("999000").unwrap();
        let ratio = (&bank / &supply) * BigDecimal::from(100);
        let ratio_f64: f64 = ratio.to_string().parse().unwrap();
        assert!(ratio_f64 < UNDER_COLLATERAL_THRESHOLD);
    }

    #[test]
    fn discrepancy_threshold_triggers_at_0_05_pct() {
        // 0.06% deviation should trigger
        let ratio_f64 = 99.94_f64;
        let deviation = (ratio_f64 - 100.0_f64).abs();
        assert!(deviation > DISCREPANCY_ALERT_THRESHOLD_PCT);
    }

    #[test]
    fn discrepancy_threshold_does_not_trigger_below_0_05_pct() {
        // 0.04% deviation should NOT trigger
        let ratio_f64 = 99.96_f64;
        let deviation = (ratio_f64 - 100.0_f64).abs();
        assert!(deviation <= DISCREPANCY_ALERT_THRESHOLD_PCT);
    }

    #[test]
    fn zero_supply_yields_100_pct_ratio() {
        let supply = BigDecimal::from(0);
        let bank = BigDecimal::from_str("1000000").unwrap();
        let ratio = if supply == BigDecimal::from(0) {
            BigDecimal::from(100)
        } else {
            (&bank / &supply) * BigDecimal::from(100)
        };
        let ratio_f64: f64 = ratio.to_string().parse().unwrap();
        assert!((ratio_f64 - 100.0).abs() < 1e-9);
    }

    #[test]
    fn bank_credentials_load_from_env_skips_incomplete_entries() {
        // No env vars set → empty list
        let banks = BankCredential::load_from_env();
        // In a clean test environment there are no RESERVE_BANK_* vars
        // so the list should be empty (or contain only fully-configured entries).
        for bank in &banks {
            assert!(!bank.label.is_empty());
            assert!(!bank.api_base_url.is_empty());
            assert!(!bank.api_key.is_empty());
            assert!(!bank.account_id.is_empty());
        }
    }
}
