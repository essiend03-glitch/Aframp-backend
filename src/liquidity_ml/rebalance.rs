//! Preemptive rebalancing trigger: evaluates inference every 15 minutes,
//! initiates funding requests if a corridor is predicted to exhaust within 6h.

use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use tokio::sync::mpsc;

use super::{
    feature_pipeline::FeatureSnapshot,
    inference::{InferenceEngine, LiquidityPrediction},
    metrics::{PREEMPTIVE_REBALANCE_TRIPS, FEATURE_STORE_BACKLOG},
};

const POLL_INTERVAL_SECS: u64 = 900; // 15 minutes
const EXHAUSTION_THRESHOLD_USD: f64 = 10_000.0; // trigger when predicted draw-down > available float

pub struct RebalanceTrigger {
    engine: InferenceEngine,
    snap_rx: mpsc::Receiver<FeatureSnapshot>,
}

impl RebalanceTrigger {
    pub fn new(engine: InferenceEngine, snap_rx: mpsc::Receiver<FeatureSnapshot>) -> Self {
        Self { engine, snap_rx }
    }

    pub async fn run(mut self) {
        loop {
            sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
            // Drain all pending snapshots
            while let Ok(snap) = self.snap_rx.try_recv() {
                FEATURE_STORE_BACKLOG.with_label_values(&[&snap.corridor_id.to_string()])
                    .set(self.snap_rx.len() as f64);
                match self.engine.predict(&snap) {
                    Ok(pred) if pred.predicted_volume > EXHAUSTION_THRESHOLD_USD => {
                        info!(
                            corridor=%pred.corridor_id,
                            predicted_usd=%pred.predicted_volume,
                            "preemptive rebalance triggered"
                        );
                        PREEMPTIVE_REBALANCE_TRIPS
                            .with_label_values(&[&pred.corridor_id.to_string()])
                            .inc();
                        // TODO: invoke interbank / smart contract funding request
                    }
                    Ok(_) => {}
                    Err(e) => warn!("inference error: {e}"),
                }
            }
        }
    }
}
