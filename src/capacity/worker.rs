/// Capacity Planning Background Worker
///
/// Schedule:
///   Every 6 hours  → run forecasts + evaluate alerts
///   Daily at 01:00 → update RCU model + project costs
///   1st of quarter → generate quarterly report
use super::engine::CapacityEngine;
use chrono::{Datelike, Timelike, Utc};
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{interval, Duration};
use tracing::{error, info};

const CHECK_INTERVAL_SECS: u64 = 6 * 3600; // 6 hours

pub struct CapacityWorker {
    engine: Arc<CapacityEngine>,
}

impl CapacityWorker {
    pub fn new(engine: Arc<CapacityEngine>) -> Self {
        Self { engine }
    }

    pub async fn run(&self, mut shutdown_rx: watch::Receiver<bool>) {
        info!("CapacityWorker started (interval = 6h)");
        let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        ticker.tick().await; // fire immediately on startup

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    info!("CapacityWorker shutting down");
                    break;
                }
                _ = ticker.tick() => {
                    self.run_cycle().await;
                }
            }
        }
    }

    async fn run_cycle(&self) {
        let now = Utc::now();

        // Always: run forecasts + evaluate alerts
        match self.engine.run_forecasts().await {
            Ok(n) => info!(forecasts = n, "Capacity forecasts updated"),
            Err(e) => error!(error = %e, "Forecast run failed"),
        }

        match self.engine.evaluate_alerts().await {
            Ok(n) => info!(alerts = n, "Capacity alerts evaluated"),
            Err(e) => error!(error = %e, "Alert evaluation failed"),
        }

        // Daily at 01:xx UTC: update RCU model + project costs
        if now.hour() == 1 {
            match self.engine.update_rcu_model().await {
                Ok(_) => info!("RCU model updated"),
                Err(e) => error!(error = %e, "RCU update failed"),
            }
            match self.engine.project_costs(12, "aws").await {
                Ok(_) => info!("Cost projections updated"),
                Err(e) => error!(error = %e, "Cost projection failed"),
            }
        }

        // Quarterly: 1st day of Jan/Apr/Jul/Oct at 02:xx UTC
        let is_quarter_start = matches!(now.month(), 1 | 4 | 7 | 10) && now.day() == 1 && now.hour() == 2;
        if is_quarter_start {
            match self.engine.generate_quarterly_report().await {
                Ok(r) => info!(quarter = %r.quarter, "Quarterly capacity report generated"),
                Err(e) => error!(error = %e, "Quarterly report generation failed"),
            }
        }
    }
}
