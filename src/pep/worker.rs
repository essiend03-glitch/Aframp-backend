//! Nightly PEP re-screening worker

use super::monitoring::PepMonitoringService;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

pub struct PepRescreeningWorker {
    monitoring: Arc<PepMonitoringService>,
}

impl PepRescreeningWorker {
    pub fn new(monitoring: Arc<PepMonitoringService>) -> Self {
        Self { monitoring }
    }

    /// Spawn the nightly re-screening loop (runs every 24 hours).
    pub fn spawn(self: Arc<Self>) {
        tokio::spawn(async move {
            // Offset by 2 hours to avoid peak traffic
            let mut ticker = interval(Duration::from_secs(24 * 3600));
            ticker.tick().await; // skip immediate first tick
            loop {
                ticker.tick().await;
                info!("PEP nightly re-screening worker triggered");
                match self.monitoring.run_nightly_rescreening().await {
                    Ok(summary) => info!(
                        screened = summary.screened_count,
                        new_matches = summary.new_matches_count,
                        status_changes = summary.status_changes_count,
                        "PEP re-screening complete"
                    ),
                    Err(e) => error!(error = %e, "PEP re-screening cycle failed"),
                }
            }
        });
    }
}
