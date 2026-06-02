use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, GaugeVec,
    HistogramVec,
};

lazy_static! {
    static ref CBDC_RPC_LATENCY: HistogramVec = register_histogram_vec!(
        "cbdc_rpc_latency_seconds",
        "Latency of CBDC DLT gateway RPC calls",
        &["gateway_name", "rpc_method"],
        vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0]
    )
    .expect("Failed to register cbdc_rpc_latency_seconds");

    static ref CROSS_RAIL_SWAP_VOLUME: CounterVec = register_counter_vec!(
        "cross_rail_swap_volume_total",
        "Total volume of cross-rail CBDC swaps",
        &["swap_type", "status", "currency"]
    )
    .expect("Failed to register cross_rail_swap_volume_total");

    static ref DLT_CONFIRMATION_BLOCKS: GaugeVec = register_gauge_vec!(
        "dlt_confirmation_blocks",
        "Number of confirmation blocks for CBDC transactions",
        &["gateway_name", "status"]
    )
    .expect("Failed to register dlt_confirmation_blocks");

    static ref TWO_PHASE_COMMIT_FAILURES: CounterVec = register_counter_vec!(
        "two_phase_commit_failures_total",
        "Number of 2PC failures by phase",
        &["phase", "reason"]
    )
    .expect("Failed to register two_phase_commit_failures_total");

    static ref CBDC_GATEWAY_HEALTH: GaugeVec = register_gauge_vec!(
        "cbdc_gateway_health_status",
        "Health status of CBDC gateways (1=healthy, 0=unhealthy)",
        &["gateway_name", "dlt_system"]
    )
    .expect("Failed to register cbdc_gateway_health_status");

    static ref CBDC_PENDING_SWAPS: GaugeVec = register_gauge_vec!(
        "cbdc_pending_swaps",
        "Number of pending CBDC swaps by status",
        &["status"]
    )
    .expect("Failed to register cbdc_pending_swaps");

    static ref CBDC_HSM_OPERATIONS: CounterVec = register_counter_vec!(
        "cbdc_hsm_operations_total",
        "Number of HSM signing operations",
        &["operation", "algorithm", "status"]
    )
    .expect("Failed to register cbdc_hsm_operations_total");

    static ref CBDC_SWAP_AMOUNT: HistogramVec = register_histogram_vec!(
        "cbdc_swap_amount",
        "Distribution of CBDC swap amounts",
        &["swap_type", "currency"],
        vec![100.0, 1000.0, 10000.0, 100000.0, 1000000.0, 10000000.0]
    )
    .expect("Failed to register cbdc_swap_amount");
}

pub struct CbdcMetrics;

impl CbdcMetrics {
    pub fn record_rpc_latency(gateway_name: &str, method: &str, latency_secs: f64) {
        CBDC_RPC_LATENCY
            .with_label_values(&[gateway_name, method])
            .observe(latency_secs);
    }

    pub fn record_swap_volume(swap_type: &str, status: &str, currency: &str) {
        CROSS_RAIL_SWAP_VOLUME
            .with_label_values(&[swap_type, status, currency])
            .inc();
    }

    pub fn record_swap_amount(swap_type: &str, currency: &str, amount: f64) {
        CBDC_SWAP_AMOUNT
            .with_label_values(&[swap_type, currency])
            .observe(amount);
    }

    pub fn update_confirmation_blocks(gateway_name: &str, status: &str, blocks: f64) {
        DLT_CONFIRMATION_BLOCKS
            .with_label_values(&[gateway_name, status])
            .set(blocks);
    }

    pub fn record_2pc_failure(phase: &str, reason: &str) {
        TWO_PHASE_COMMIT_FAILURES
            .with_label_values(&[phase, reason])
            .inc();
    }

    pub fn update_gateway_health(gateway_name: &str, dlt_system: &str, healthy: bool) {
        let val = if healthy { 1.0 } else { 0.0 };
        CBDC_GATEWAY_HEALTH
            .with_label_values(&[gateway_name, dlt_system])
            .set(val);
    }

    pub fn set_pending_swaps(status: &str, count: f64) {
        CBDC_PENDING_SWAPS
            .with_label_values(&[status])
            .set(count);
    }

    pub fn record_hsm_operation(operation: &str, algorithm: &str, status: &str) {
        CBDC_HSM_OPERATIONS
            .with_label_values(&[operation, algorithm, status])
            .inc();
    }
}
