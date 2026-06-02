//! Prometheus metrics for Issue #533.

use lazy_static::lazy_static;
use prometheus::{register_counter_vec, register_gauge_vec, register_histogram_vec,
    CounterVec, GaugeVec, HistogramVec};

lazy_static! {
    pub static ref BFT_CONSENSUS_LATENCY: HistogramVec = register_histogram_vec!(
        "oracle_bft_consensus_latency_ms",
        "Time to reach BFT consensus on a price tick",
        &["pair"],
        vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0]
    ).unwrap();

    pub static ref NODE_BYZANTINE_VARIANCE: GaugeVec = register_gauge_vec!(
        "node_byzantine_variance_bps",
        "Basis-point deviation of each oracle node from consensus price",
        &["node_id"]
    ).unwrap();

    pub static ref MEV_ATTACKS_BLOCKED: CounterVec = register_counter_vec!(
        "mev_attacks_blocked_count",
        "Number of MEV / flash-loan attacks intercepted",
        &["pair", "action"]
    ).unwrap();

    pub static ref AGGREGATED_PRICE_DRIFT: GaugeVec = register_gauge_vec!(
        "aggregated_price_drift_ratio",
        "Ratio of consensus price drift vs previous window",
        &["pair"]
    ).unwrap();
}
