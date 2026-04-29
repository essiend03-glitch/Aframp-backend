/// Monthly SLA compliance report generator.
///
/// Runs on the 1st of each month (or on demand via the API) and produces
/// a `sla_compliance_reports` row for every partner and one platform-wide row.
use crate::sla::repository::SlaRepository;
use chrono::{Datelike, NaiveDate, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{interval, Duration};
use tracing::{error, info};

/// Check every hour; generate when the day-of-month rolls to 1.
const CHECK_INTERVAL_SECS: u64 = 3600;

pub struct SlaReportWorker {
    repo: Arc<SlaRepository>,
    pool: PgPool,
}

impl SlaReportWorker {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: Arc::new(SlaRepository::new(pool.clone())),
            pool,
        }
    }

    pub async fn run(self, mut shutdown_rx: watch::Receiver<bool>) {
        info!("SlaReportWorker started");
        let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => { break; }
                _ = ticker.tick() => {
                    let now = Utc::now();
                    // Generate on the 1st of each month
                    if now.day() == 1 && now.hour() < 2 {
                        let prev = {
                            let (y, m) = if now.month() == 1 {
                                (now.year() - 1, 12u32)
                            } else {
                                (now.year(), now.month() - 1)
                            };
                            NaiveDate::from_ymd_opt(y, m, 1).unwrap()
                        };
                        if let Err(e) = self.generate_for_month(prev).await {
                            error!(error = %e, month = %prev, "SLA report generation failed");
                        }
                    }
                }
            }
        }
    }

    pub async fn generate_for_month(&self, month: NaiveDate) -> anyhow::Result<()> {
        // Platform-wide report
        let report = self.repo.generate_monthly_report(None, month).await?;
        info!(
            month = %month,
            total_breaches = report.total_breaches,
            availability_pct = ?report.availability_pct,
            "Platform SLA report generated"
        );

        // Per-partner reports — collect distinct partner IDs from incidents
        let partner_ids: Vec<uuid::Uuid> = sqlx::query_scalar!(
            r#"SELECT DISTINCT p.id FROM partners p
               JOIN sla_breach_incidents i ON TRUE
               WHERE i.detected_at >= $1 AND i.detected_at < $2"#,
            month.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            {
                let (y, m) = if month.month() == 12 {
                    (month.year() + 1, 1u32)
                } else {
                    (month.year(), month.month() + 1)
                };
                NaiveDate::from_ymd_opt(y, m, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc()
            }
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        for pid in partner_ids {
            let _ = self.repo.generate_monthly_report(Some(pid), month).await;
        }

        Ok(())
    }
}
