//! Analytics snapshot background worker (Issue #369).
//!
//! Runs on a configurable schedule and incrementally computes wallet usage
//! snapshots for all active wallets. Only processes activity since the last
//! snapshot timestamp for each wallet (incremental computation).

use std::sync::Arc;
use std::time::Duration;

use chrono::{Datelike, Duration as CDuration, Timelike, Utc};
use sqlx::PgPool;
use tokio::sync::watch;
use tracing::{error, info, warn};

use crate::database::analytics_repository::AnalyticsRepository;
use crate::metrics::analytics as metrics;
use crate::services::analytics::{AnalyticsConfig, AnalyticsService};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SnapshotWorkerConfig {
    /// How often the worker wakes up to check for pending snapshots.
    pub poll_interval: Duration,
    /// Compute daily snapshots within this many seconds of period end.
    pub daily_deadline_secs: u64,
    /// Compute weekly snapshots within this many seconds of period end.
    pub weekly_deadline_secs: u64,
    /// Compute monthly snapshots within this many seconds of period end.
    pub monthly_deadline_secs: u64,
    /// Multiplier for anomaly detection (passed to AnalyticsConfig).
    pub volume_spike_multiplier: f64,
}

impl Default for SnapshotWorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(300), // 5 min
            daily_deadline_secs: 3600,               // 1 hour
            weekly_deadline_secs: 7200,              // 2 hours
            monthly_deadline_secs: 14400,            // 4 hours
            volume_spike_multiplier: 3.0,
        }
    }
}

impl SnapshotWorkerConfig {
    pub fn from_env() -> Self {
        let poll = std::env::var("ANALYTICS_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
        Self {
            poll_interval: Duration::from_secs(poll),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

pub struct AnalyticsSnapshotWorker {
    service: Arc<AnalyticsService>,
    repo: Arc<AnalyticsRepository>,
    config: SnapshotWorkerConfig,
}

impl AnalyticsSnapshotWorker {
    pub fn new(pool: PgPool, config: SnapshotWorkerConfig) -> Self {
        let analytics_config = AnalyticsConfig {
            volume_spike_multiplier: config.volume_spike_multiplier,
            ..Default::default()
        };
        let service = Arc::new(AnalyticsService::new(pool.clone(), analytics_config));
        let repo = service.repo();
        Self { service, repo, config }
    }

    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        info!("Analytics snapshot worker started");
        let mut interval = tokio::time::interval(self.config.poll_interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.run_cycle().await;
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Analytics snapshot worker shutting down");
                        break;
                    }
                }
            }
        }
    }

    async fn run_cycle(&self) {
        let now = Utc::now();
        info!(cycle_at = %now, "Analytics snapshot worker cycle");

        // Fetch all wallets active in the last 30 days
        let since = now - CDuration::days(30);
        let wallets = match self.repo.active_wallet_addresses(since).await {
            Ok(w) => w,
            Err(e) => {
                error!(error=%e, "Failed to fetch active wallets");
                return;
            }
        };

        info!(wallet_count = wallets.len(), "Processing analytics snapshots");

        for wallet in &wallets {
            // Daily snapshot
            self.maybe_compute_snapshot(wallet, "daily", now).await;
            // Weekly snapshot
            self.maybe_compute_snapshot(wallet, "weekly", now).await;
            // Monthly snapshot
            self.maybe_compute_snapshot(wallet, "monthly", now).await;

            // Behaviour profile (weekly update)
            self.service.compute_profile(wallet).await;

            // Anomaly detection
            self.service.detect_anomalies(wallet).await;

            // Insights
            self.service.generate_insights(wallet, "weekly").await;
            self.service.generate_insights(wallet, "monthly").await;

            metrics::snapshot_generated(wallet, "all");
        }

        // Compute admin daily aggregate for today
        self.compute_admin_aggregate(now).await;

        // Update Prometheus gauges
        let open_anomalies = self.repo.count_open_anomalies().await.unwrap_or(0);
        metrics::anomaly_flagged_wallets(open_anomalies as f64);
        metrics::active_wallet_count(wallets.len() as f64);

        let avg_risk = self.repo.avg_risk_score().await.unwrap_or(0.0);
        metrics::avg_risk_score(avg_risk);
    }

    async fn maybe_compute_snapshot(
        &self,
        wallet: &str,
        period: &str,
        now: chrono::DateTime<Utc>,
    ) {
        let (period_start, period_end) = period_bounds(period, now);

        // Incremental: only recompute if we haven't snapshotted this period yet
        // or if the last snapshot was before the period ended.
        let last = self.repo.get_latest_snapshot_at(wallet, period).await.unwrap_or(None);
        if let Some(last_at) = last {
            if last_at >= period_end {
                return; // already up to date
            }
        }

        self.service.compute_snapshot(wallet, period, period_start, period_end).await;
    }

    async fn compute_admin_aggregate(&self, now: chrono::DateTime<Utc>) {
        use bigdecimal::BigDecimal;
        use sqlx::types::BigDecimal as BD;

        let today = now.date_naive();
        let from = now - CDuration::days(1);

        // Count active wallets today
        let active = self.repo.active_wallet_addresses(from).await.unwrap_or_default();

        // Aggregate from daily snapshots
        let snaps = self.repo
            .get_snapshots("", "daily", from, now)
            .await
            .unwrap_or_default();

        let total_cngn: BD = snaps.iter().map(|s| s.total_cngn_sent.clone()).sum();
        let total_onramped: BD = snaps.iter().map(|s| s.total_fiat_onramped.clone()).sum();
        let total_offramped: BD = snaps.iter().map(|s| s.total_fiat_offramped.clone()).sum();
        let total_txs: i64 = snaps.iter().map(|s| s.total_tx_count as i64).sum();
        let avg_size = if !snaps.is_empty() {
            snaps.iter().map(|s| s.total_cngn_sent.clone()).sum::<BD>()
                / BD::from(snaps.len() as i64)
        } else {
            BD::from(0)
        };

        let _ = self.repo.upsert_daily_aggregate(
            today,
            0, // total wallets — would need a separate count query
            active.len() as i64,
            0,
            total_cngn,
            total_onramped,
            total_offramped,
            avg_size,
            total_txs,
        ).await;
    }
}

// ---------------------------------------------------------------------------
// Period boundary helpers
// ---------------------------------------------------------------------------

fn period_bounds(
    period: &str,
    now: chrono::DateTime<Utc>,
) -> (chrono::DateTime<Utc>, chrono::DateTime<Utc>) {
    match period {
        "daily" => {
            let start = now
                .with_hour(0).unwrap_or(now)
                .with_minute(0).unwrap_or(now)
                .with_second(0).unwrap_or(now)
                .with_nanosecond(0).unwrap_or(now);
            let end = start + CDuration::days(1);
            (start, end)
        }
        "weekly" => {
            use chrono::Weekday;
            let days_since_monday = now.weekday().num_days_from_monday() as i64;
            let start = (now - CDuration::days(days_since_monday))
                .with_hour(0).unwrap_or(now)
                .with_minute(0).unwrap_or(now)
                .with_second(0).unwrap_or(now)
                .with_nanosecond(0).unwrap_or(now);
            let end = start + CDuration::weeks(1);
            (start, end)
        }
        _ => {
            // monthly
            let start = now
                .with_day(1).unwrap_or(now)
                .with_hour(0).unwrap_or(now)
                .with_minute(0).unwrap_or(now)
                .with_second(0).unwrap_or(now)
                .with_nanosecond(0).unwrap_or(now);
            // End = first day of next month
            let next_month = if now.month() == 12 {
                start.with_year(now.year() + 1).and_then(|d| d.with_month(1)).unwrap_or(start)
            } else {
                start.with_month(now.month() + 1).unwrap_or(start)
            };
            (start, next_month)
        }
    }
}
