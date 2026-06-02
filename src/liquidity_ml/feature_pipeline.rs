//! Streaming feature engineering pipeline: rolling velocity, variance, context enrichment.
//! Uses an in-memory ring buffer to isolate aggregation from the transaction hot path.

use std::{collections::VecDeque, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use tracing::debug;
use uuid::Uuid;

const RING_CAPACITY: usize = 1024;

/// A single payment frame ingested from the core routing layer.
#[derive(Debug, Clone)]
pub struct PaymentFrame {
    pub corridor_id:  Uuid,
    pub amount_usd:   f64,  // 7 decimal places
    pub currency:     String,
    pub timestamp_ms: i64,
    pub bank_delay_ms: i32,
}

/// Aggregated feature snapshot for a corridor (sent to inference engine).
#[derive(Debug, Clone)]
pub struct FeatureSnapshot {
    pub corridor_id:     Uuid,
    pub throughput_usd:  f64,
    pub rolling_variance: f64,
    pub velocity_1h:     f64,
    pub velocity_24h:    f64,
    pub bank_delay_ms:   f64,
}

/// Ring-buffer accumulator per corridor.
struct CorridorBuffer {
    frames: VecDeque<PaymentFrame>,
}

impl CorridorBuffer {
    fn new() -> Self { Self { frames: VecDeque::with_capacity(RING_CAPACITY) } }

    fn push(&mut self, frame: PaymentFrame) {
        if self.frames.len() >= RING_CAPACITY { self.frames.pop_front(); }
        self.frames.push_back(frame);
    }

    fn compute_snapshot(&self, corridor_id: Uuid) -> FeatureSnapshot {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let amounts: Vec<f64> = self.frames.iter().map(|f| f.amount_usd).collect();
        let throughput = amounts.iter().sum::<f64>();
        let mean = if amounts.is_empty() { 0.0 } else { throughput / amounts.len() as f64 };
        let variance = amounts.iter().map(|a| (a - mean).powi(2)).sum::<f64>()
            / amounts.len().max(1) as f64;

        let window_1h = now_ms - 3_600_000;
        let window_24h = now_ms - 86_400_000;

        let vel_1h:  f64 = self.frames.iter().filter(|f| f.timestamp_ms >= window_1h).map(|f| f.amount_usd).sum();
        let vel_24h: f64 = self.frames.iter().filter(|f| f.timestamp_ms >= window_24h).map(|f| f.amount_usd).sum();
        let avg_delay = self.frames.iter().map(|f| f.bank_delay_ms as f64).sum::<f64>()
            / self.frames.len().max(1) as f64;

        FeatureSnapshot { corridor_id, throughput_usd: throughput, rolling_variance: variance,
            velocity_1h: vel_1h, velocity_24h: vel_24h, bank_delay_ms: avg_delay }
    }
}

/// Actor-style pipeline: receives frames, maintains per-corridor ring buffers,
/// forwards snapshots to the inference engine channel.
pub struct FeaturePipeline {
    tx_in:    mpsc::Sender<PaymentFrame>,
    snapshot_tx: mpsc::Sender<FeatureSnapshot>,
}

impl FeaturePipeline {
    pub fn spawn(snapshot_tx: mpsc::Sender<FeatureSnapshot>) -> Self {
        let (tx_in, mut rx_in) = mpsc::channel::<PaymentFrame>(4096);
        let buffers: Arc<RwLock<std::collections::HashMap<Uuid, CorridorBuffer>>> =
            Arc::new(RwLock::new(std::collections::HashMap::new()));

        let snap_tx = snapshot_tx.clone();
        tokio::spawn(async move {
            while let Some(frame) = rx_in.recv().await {
                let cid = frame.corridor_id;
                let mut map = buffers.write().await;
                let buf = map.entry(cid).or_insert_with(CorridorBuffer::new);
                buf.push(frame);
                let snap = buf.compute_snapshot(cid);
                drop(map);
                debug!(corridor=%cid, "feature snapshot computed");
                let _ = snap_tx.try_send(snap);
            }
        });

        Self { tx_in, snapshot_tx }
    }

    pub async fn ingest(&self, frame: PaymentFrame) -> bool {
        self.tx_in.try_send(frame).is_ok()
    }
}
