//! Background worker that computes DeFi analytics snapshots on a configurable schedule.
//! Alerts if the job does not complete within the configured interval.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{error, info, warn};

use super::service::DefiAnalyticsService;

#[derive(Debug, Clone)]
pub struct DefiAnalyticsWorkerConfig {
    /// How often to run the platform snapshot job (seconds)
    pub snapshot_interval_secs: u64,
    /// Alert threshold: if job takes longer than this, log a warning
    pub alert_threshold_secs: u64,
    /// TVL drop alert threshold (percentage, e.g. 0.10 = 10%)
    pub tvl_drop_alert_pct: f64,
}

impl Default for DefiAnalyticsWorkerConfig {
    fn default() -> Self {
        Self {
            snapshot_interval_secs: std::env::var("DEFI_ANALYTICS_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            alert_threshold_secs: 300,
            tvl_drop_alert_pct: std::env::var("DEFI_TVL_DROP_ALERT_PCT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.10),
        }
    }
}

pub struct DefiAnalyticsWorker {
    svc: Arc<DefiAnalyticsService>,
    config: DefiAnalyticsWorkerConfig,
}

impl DefiAnalyticsWorker {
    pub fn new(svc: Arc<DefiAnalyticsService>, config: DefiAnalyticsWorkerConfig) -> Self {
        Self { svc, config }
    }

    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let interval = Duration::from_secs(self.config.snapshot_interval_secs);
        let mut ticker = tokio::time::interval(interval);
        let mut prev_tvl: Option<f64> = None;

        info!(
            interval_secs = self.config.snapshot_interval_secs,
            "DeFi analytics worker started"
        );

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let start = std::time::Instant::now();

                    match self.svc.compute_platform_snapshot().await {
                        Ok(snapshot) => {
                            let elapsed = start.elapsed();
                            let tvl: f64 = snapshot.total_value_locked.to_string().parse().unwrap_or(0.0);

                            // Alert if job took too long
                            if elapsed.as_secs() > self.config.alert_threshold_secs {
                                warn!(
                                    elapsed_secs = elapsed.as_secs(),
                                    threshold_secs = self.config.alert_threshold_secs,
                                    "DeFi analytics snapshot job exceeded schedule threshold"
                                );
                            }

                            // Alert on TVL drop
                            if let Some(prev) = prev_tvl {
                                if prev > 0.0 {
                                    let drop_pct = (prev - tvl) / prev;
                                    if drop_pct > self.config.tvl_drop_alert_pct {
                                        warn!(
                                            prev_tvl = prev,
                                            current_tvl = tvl,
                                            drop_pct = drop_pct,
                                            threshold_pct = self.config.tvl_drop_alert_pct,
                                            "⚠️  DeFi TVL dropped beyond alert threshold — possible unusual withdrawal activity"
                                        );
                                    }
                                }
                            }
                            prev_tvl = Some(tvl);

                            // Also compute lending snapshot
                            if let Err(e) = self.svc.compute_lending_snapshot().await {
                                error!(error = %e, "Failed to compute lending snapshot");
                            }

                            info!(elapsed_ms = elapsed.as_millis(), "DeFi analytics snapshot completed");
                        }
                        Err(e) => {
                            error!(error = %e, "DeFi analytics snapshot failed");
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("DeFi analytics worker shutting down");
                        break;
                    }
                }
            }
        }
    }
}
