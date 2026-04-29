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
use bigdecimal::BigDecimal;
use chrono::Utc;
use ed25519_dalek::Signer;
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{error, info, instrument, warn};

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
}

impl ProofOfReservesWorker {
    pub fn new(
        pool: PgPool,
        stellar_client: StellarClient,
        signing_key: Arc<ed25519_dalek::SigningKey>,
        audit_writer: Option<Arc<AuditWriter>>,
        cngn_issuer: String,
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
        let total_on_chain_supply = self.fetch_on_chain_supply().await?;

        // 2. Bank balances
        let bank_balances = self.fetch_bank_balances().await;

        // 3. Aggregate bank assets
        let total_bank_assets: BigDecimal = bank_balances
            .iter()
            .fold(BigDecimal::from(0), |acc, b| acc + &b.settled_balance);

        // Custodian solvency timestamp: earliest balance_as_of across all banks
        // (most conservative — all banks must have confirmed by this time).
        let custodian_solvency_ts = bank_balances
            .iter()
            .map(|b| b.balance_as_of)
            .min()
            .unwrap_or_else(Utc::now);

        // 4. Collateralization ratio
        let ratio_pct = if total_on_chain_supply == BigDecimal::from(0) {
            // No supply → trivially 100% backed
            BigDecimal::from(100)
        } else {
            (&total_bank_assets / &total_on_chain_supply) * BigDecimal::from(100)
        };
        let ratio_f64: f64 = ratio_pct.to_string().parse().unwrap_or(0.0);

        let is_fully_collateralized = ratio_f64 >= UNDER_COLLATERAL_THRESHOLD;

        // 5. Build and sign canonical payload
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

        // 6. Persist snapshot
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
            )
            .await?;

        info!(
            snapshot_id = %snapshot_id,
            supply = %total_on_chain_supply,
            bank_assets = %total_bank_assets,
            ratio_pct = ratio_f64,
            fully_collateralized = is_fully_collateralized,
            "PoR snapshot recorded"
        );

        // 7. Under-collateralization alert (ratio < 100.01%)
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

        // 8. Discrepancy investigation alert (deviation > 0.05% from 100.00%)
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

        let amount_str = stats
            .get("amount")
            .and_then(|v| v.as_str())
            .unwrap_or("0");

        Ok(BigDecimal::from_str(amount_str).unwrap_or_else(|_| BigDecimal::from(0)))
    }

    // ── Bank balances ─────────────────────────────────────────────────────────

    /// Fetch settled balances from all configured custodian banks.
    /// Uses read-only credentials — cannot initiate transfers.
    /// Failures are logged but do not abort the cycle; a missing bank simply
    /// contributes 0 to the total (which will trigger an under-collateral alert).
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
                    // A missing bank balance is treated as 0, which will
                    // naturally trigger an under-collateral alert.
                    results.push(BankBalance {
                        label: bank.label.clone(),
                        settled_balance: BigDecimal::from(0),
                        currency: "NGN".to_string(),
                        balance_as_of: Utc::now(),
                    });
                }
            }
        }

        // If no banks are configured, log a warning.
        if results.is_empty() {
            warn!("No custodian bank credentials configured (RESERVE_BANK_1_* env vars). PoR bank total will be 0.");
        }

        results
    }

    /// Fetch a single bank's settled balance via its read-only API.
    ///
    /// Expected JSON response shape:
    /// ```json
    /// {
    ///   "settled_balance": "1234567890.00",
    ///   "currency": "NGN",
    ///   "balance_as_of": "2026-04-23T12:00:00Z"
    /// }
    /// ```
    async fn fetch_single_bank_balance(&self, bank: &BankCredential) -> anyhow::Result<BankBalance> {
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

        Ok(BankBalance {
            label: bank.label.clone(),
            settled_balance: BigDecimal::from_str(balance_str)
                .unwrap_or_else(|_| BigDecimal::from(0)),
            currency,
            balance_as_of,
        })
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
    ) -> anyhow::Result<uuid::Uuid> {
        let mut tx = self.pool.begin().await?;

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

        // Write to Audit Trail (Issue #117)
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
