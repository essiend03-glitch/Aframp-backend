//! Banking Integration — Daily Reconciliation Engine (Issue #407)
//!
//! Compares Aframp's internal transfer ledger against the bank's end-of-day
//! statement for each active bank code. Flags discrepancies for human review.

use super::repository::BankingRepository;
use chrono::{NaiveDate, Utc};
use reqwest::Client as HttpClient;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

pub struct ReconciliationEngine {
    repo: Arc<BankingRepository>,
    http: HttpClient,
}

impl ReconciliationEngine {
    pub fn new(repo: Arc<BankingRepository>) -> Self {
        Self {
            repo,
            http: HttpClient::new(),
        }
    }

    /// Run reconciliation for all active bank codes for the given date.
    /// Called daily (e.g. by a cron worker at 01:00 UTC after EOD statements arrive).
    #[instrument(skip(self), fields(date = %date))]
    pub async fn run_for_date(&self, date: NaiveDate) -> anyhow::Result<Vec<ReconciliationResult>> {
        let bank_codes = self.get_active_bank_codes().await?;
        let mut results = Vec::with_capacity(bank_codes.len());

        for bank_code in bank_codes {
            match self.reconcile_bank(&bank_code, date).await {
                Ok(r) => {
                    info!(
                        bank_code = %bank_code,
                        status = %r.status,
                        discrepancy = %r.discrepancy,
                        "Reconciliation complete"
                    );
                    results.push(r);
                }
                Err(e) => {
                    error!(bank_code = %bank_code, error = %e, "Reconciliation failed for bank");
                }
            }
        }

        Ok(results)
    }

    /// Reconcile a single bank for a given date.
    async fn reconcile_bank(
        &self,
        bank_code: &str,
        date: NaiveDate,
    ) -> anyhow::Result<ReconciliationResult> {
        // 1. Aframp internal total (settled transfers for this bank on this date)
        let aframp_total = self.repo.sum_settled_transfers(bank_code, date).await?;

        // 2. Bank EOD statement total (fetched from provider API)
        let bank_total = self.fetch_bank_statement_total(bank_code, date).await
            .unwrap_or_else(|e| {
                warn!(bank_code = %bank_code, error = %e, "Could not fetch bank statement; using 0");
                BigDecimal::from(0)
            });

        // 3. Compute discrepancy
        let discrepancy = &aframp_total - &bank_total;
        let abs_discrepancy = if discrepancy < BigDecimal::from(0) {
            -discrepancy.clone()
        } else {
            discrepancy.clone()
        };

        let (status, flagged_count) = if abs_discrepancy == BigDecimal::from(0) {
            ("equilibrium", 0)
        } else if abs_discrepancy > BigDecimal::from(100_000) {
            // > 100,000 kobo (₦1,000) — critical discrepancy
            ("discrepancy", 1)
        } else {
            // Minor rounding / timing difference
            ("pending_review", 1)
        };

        let metadata = serde_json::json!({
            "bank_code": bank_code,
            "date": date.to_string(),
            "aframp_total_kobo": aframp_total,
            "bank_total_kobo": bank_total,
            "discrepancy_kobo": discrepancy,
        });

        let run = self
            .repo
            .upsert_reconciliation_run(
                date,
                bank_code,
                &aframp_total,
                &bank_total,
                &abs_discrepancy,
                flagged_count,
                status,
                Some(&metadata),
            )
            .await?;

        Ok(ReconciliationResult {
            run_id: run.id,
            bank_code: bank_code.to_string(),
            date,
            aframp_total,
            bank_total,
            discrepancy: abs_discrepancy,
            status: status.to_string(),
        })
    }

    /// Fetch EOD statement total from the bank/provider API.
    /// Falls back to 0 if the provider is unavailable (bank downtime must not crash core services).
    async fn fetch_bank_statement_total(
        &self,
        bank_code: &str,
        date: NaiveDate,
    ) -> anyhow::Result<BigDecimal> {
        let paystack_key = std::env::var("PAYSTACK_SECRET_KEY")
            .map_err(|_| anyhow::anyhow!("PAYSTACK_SECRET_KEY not configured"))?;

        let url = format!(
            "https://api.paystack.co/transaction?status=success&from={}&to={}&perPage=500",
            date.format("%Y-%m-%d"),
            date.format("%Y-%m-%d"),
        );

        let resp = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.http.get(&url).bearer_auth(&paystack_key).send(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Bank statement fetch timed out"))??;

        if !resp.status().is_success() {
            anyhow::bail!("Provider returned {}", resp.status());
        }

        let json: serde_json::Value = resp.json().await?;
        let total = json
            .pointer("/meta/total_volume")
            .and_then(|v| v.as_str())
            .and_then(|s| BigDecimal::from_str(s).ok())
            .unwrap_or_else(|| BigDecimal::from(0));

        Ok(total)
    }

    /// Return distinct bank codes from active linked accounts.
    async fn get_active_bank_codes(&self) -> anyhow::Result<Vec<String>> {
        // Delegate to a raw query via the pool exposed by the repository
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT bank_code FROM linked_bank_accounts WHERE status = 'active'",
        )
        .fetch_all(self.repo.pool())
        .await?;
        Ok(rows.into_iter().map(|(c,)| c).collect())
    }
}

#[derive(Debug)]
pub struct ReconciliationResult {
    pub run_id: uuid::Uuid,
    pub bank_code: String,
    pub date: NaiveDate,
    pub aframp_total: BigDecimal,
    pub bank_total: BigDecimal,
    pub discrepancy: BigDecimal,
    pub status: String,
}
