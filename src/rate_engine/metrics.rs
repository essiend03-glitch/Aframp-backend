//! Prometheus metrics for Issue #530 observability requirements.

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec,
    CounterVec, GaugeVec, HistogramVec,
};

lazy_static! {
    pub static ref THROTTLES_TOTAL: CounterVec = register_counter_vec!(
        "tenant_rate_limit_throttles_total",
        "Total HTTP 429 responses issued per tenant",
        &["tenant_id", "tier"]
    ).unwrap();

    pub static ref SCHEDULER_DEFICIT_QUANTUMS: GaugeVec = register_gauge_vec!(
        "scheduler_deficit_quantums",
        "Current DRR deficit quantum balance per tenant",
        &["tenant_id"]
    ).unwrap();

    pub static ref QUEUE_BACKLOG_DEPTH: GaugeVec = register_gauge_vec!(
        "queue_backlog_depth_count",
        "Outstanding items in the tenant processing queue",
        &["tenant_id"]
    ).unwrap();

    pub static ref PROCESSING_LATENCY: HistogramVec = register_histogram_vec!(
        "tenant_processing_latency_seconds",
        "End-to-end transaction processing latency per tenant",
        &["tenant_id"],
        vec![0.001, 0.003, 0.005, 0.010, 0.025, 0.050, 0.100, 0.250]
    ).unwrap();
}
