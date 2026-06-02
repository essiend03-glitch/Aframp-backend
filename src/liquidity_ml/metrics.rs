//! Prometheus metrics for Issue #531.

use lazy_static::lazy_static;
use prometheus::{register_counter_vec, register_gauge_vec, register_histogram_vec,
    CounterVec, GaugeVec, HistogramVec};

lazy_static! {
    pub static ref INFERENCE_LATENCY_MS: HistogramVec = register_histogram_vec!(
        "inference_latency_ms",
        "ML forward-pass latency in milliseconds",
        &["corridor_id"],
        vec![1.0, 5.0, 10.0, 20.0, 50.0, 100.0]
    ).unwrap();

    pub static ref PREDICTION_ERROR_MAE: GaugeVec = register_gauge_vec!(
        "model_prediction_error_mae",
        "Mean absolute error between predicted and actual volume",
        &["corridor_id"]
    ).unwrap();

    pub static ref PREEMPTIVE_REBALANCE_TRIPS: CounterVec = register_counter_vec!(
        "preemptive_rebalance_trips_total",
        "Number of preemptive rebalancing requests triggered",
        &["corridor_id"]
    ).unwrap();

    pub static ref FEATURE_STORE_BACKLOG: GaugeVec = register_gauge_vec!(
        "feature_store_backlog_depth",
        "Depth of pending feature snapshots awaiting inference",
        &["corridor_id"]
    ).unwrap();
}
