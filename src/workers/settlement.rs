//! Daily net-settlement worker for remittance partners (Issue #408).
//!
//! Runs once per day (configurable), computes net-settlement for every active
//! partner, generates a CSV report, and marks the settlement as sent.

use std::sync::Arc;
use std::time::Duration;

use chrono::{Timelike, Utc};
use sqlx::PgPool;
use tokio::sync::watch;
use tracing::{error, info, warn};

use crate::database::partner_repository::PartnerRepository;
use crate::services::partner::PartnerService;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SettlementWorkerConfig {
    /// Hour of day (UTC) at which to run settlement (default: 23 = 11 PM UTC).
    pub run_hour_utc: u32,
    /// How often to check if it's time to run (default: 5 min).
    pub poll_interval: Duration,
}

impl Default for SettlementWorkerConfig {
    fn default() -> Self {
        Self {
            run_hour_utc: 23,
            poll_interval: Duration::from_secs(300),
        }
    }
}

impl SettlementWorkerConfig {
    pub fn from_env() -> Self {
        let hour = std::env::var("SETTLEMENT_RUN_HOUR_UTC")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(23);
        Self { run_hour_utc: hour, ..Default::default() }
    }
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

pub struct SettlementWorker {
    repo: Arc<PartnerRepository>,
    service: Arc<PartnerService>,
    config: SettlementWorkerConfig,
    last_run_date: Option<chrono::NaiveDate>,
}

impl SettlementWorker {
    pub fn new(pool: PgPool, config: SettlementWorkerConfig) -> Self {
        let repo = Arc::new(PartnerRepository::new(pool));
        let service = Arc::new(PartnerService::new(repo.clone()));
        Self { repo, service, config, last_run_date: None }
    }

    pub async fn run(mut self, mut shutdown: watch::Receiver<bool>) {
        info!("Settlement worker started (run_hour_utc={})", self.config.run_hour_utc);
        let mut interval = tokio::time::interval(self.config.poll_interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let now = Utc::now();
                    let today = now.date_naive();
                    // Run once per day at the configured hour
                    if now.hour() >= self.config.run_hour_utc
                        && self.last_run_date != Some(today)
                    {
                        self.run_settlement(today).await;
                        self.last_run_date = Some(today);
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Settlement worker shutting down");
                        break;
                    }
                }
            }
        }
    }

    async fn run_settlement(&self, date: chrono::NaiveDate) {
        info!(date=%date, "Running daily settlement");

        let partners = match self.repo.list_partners().await {
            Ok(p) => p,
            Err(e) => { error!(error=%e, "Failed to list partners for settlement"); return; }
        };

        for partner in partners.iter().filter(|p| p.status == "active") {
            if let Err(e) = self.service.compute_settlement(partner.id, date).await {
                warn!(partner_id=%partner.id, error=%e, "Settlement computation failed");
                continue;
            }

            // Generate CSV report
            let report = self.generate_csv_report(partner.id, date).await;

            // Mark settlement sent (report_url is a placeholder — in production
            // this would be an S3 pre-signed URL or email attachment)
            let settlements = self.repo.list_settlements(partner.id, 1).await.unwrap_or_default();
            if let Some(s) = settlements.first() {
                let report_url = format!("data:text/csv;base64,{}", base64_encode(&report));
                if let Err(e) = self.repo.mark_settlement_sent(s.id, &report_url).await {
                    warn!(settlement_id=%s.id, error=%e, "Failed to mark settlement sent");
                } else {
                    info!(partner=%partner.slug, date=%date, "Settlement report sent");
                }
            }
        }
    }

    async fn generate_csv_report(&self, partner_id: uuid::Uuid, date: chrono::NaiveDate) -> String {
        let (volume, fees, tx_count) = self.repo
            .daily_transfer_summary(partner_id, date)
            .await
            .unwrap_or_default();

        format!(
            "settlement_date,total_volume,total_fees,net_payable,tx_count\n{},{},{},{},{}\n",
            date,
            volume,
            fees,
            fees, // net_payable = fees kept by Aframp
            tx_count,
        )
    }
}

fn base64_encode(s: &str) -> String {
    use std::fmt::Write;
    // Minimal base64 without external dep — just use hex for the placeholder
    let mut out = String::new();
    for b in s.bytes() {
        let _ = write!(out, "{:02x}", b);
    }
    out
}
