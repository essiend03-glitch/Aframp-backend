//! Prometheus metrics for Stellar Ecosystem Partner Integration (Issue #470).

#[cfg(feature = "database")]
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, GaugeVec,
    HistogramVec,
};
#[cfg(feature = "database")]
use std::sync::OnceLock;

// ─────────────────────────────────────────────────────────────────────────────
// Metric statics
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
static DEX_SLIPPAGE_REJECTIONS: OnceLock<CounterVec> = OnceLock::new();
#[cfg(feature = "database")]
static PATHFINDING_DURATION: OnceLock<HistogramVec> = OnceLock::new();
#[cfg(feature = "database")]
static ANCHOR_SEP_API_LATENCY: OnceLock<HistogramVec> = OnceLock::new();
#[cfg(feature = "database")]
static CROSS_ANCHOR_TRANSFERS_TOTAL: OnceLock<CounterVec> = OnceLock::new();
#[cfg(feature = "database")]
static ANCHOR_TRUSTLINE_RESERVE_LOW: OnceLock<GaugeVec> = OnceLock::new();
#[cfg(feature = "database")]
static PATHFINDING_FAILURES_CONSECUTIVE: OnceLock<GaugeVec> = OnceLock::new();

// ─────────────────────────────────────────────────────────────────────────────
// Accessors
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
fn dex_slippage_rejections() -> &'static CounterVec {
    DEX_SLIPPAGE_REJECTIONS.get_or_init(|| {
        register_counter_vec!(
            "stellar_dex_slippage_rejections_total",
            "Total DEX path-payment executions aborted due to slippage exceeding threshold",
            &["base_asset", "counter_asset"]
        )
        .expect("register stellar_dex_slippage_rejections_total")
    })
}

#[cfg(feature = "database")]
fn pathfinding_duration() -> &'static HistogramVec {
    PATHFINDING_DURATION.get_or_init(|| {
        register_histogram_vec!(
            "stellar_pathfinding_duration_seconds",
            "Latency of Stellar DEX pathfinding queries",
            &["base_asset", "counter_asset"],
            vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.2, 0.5, 1.0]
        )
        .expect("register stellar_pathfinding_duration_seconds")
    })
}

#[cfg(feature = "database")]
fn anchor_sep_api_latency() -> &'static HistogramVec {
    ANCHOR_SEP_API_LATENCY.get_or_init(|| {
        register_histogram_vec!(
            "anchor_sep_api_latency_seconds",
            "Latency of outbound SEP-24/SEP-31 API calls to partner anchors",
            &["anchor_domain", "sep_protocol", "operation"],
            vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
        )
        .expect("register anchor_sep_api_latency_seconds")
    })
}

#[cfg(feature = "database")]
fn cross_anchor_transfers_total() -> &'static CounterVec {
    CROSS_ANCHOR_TRANSFERS_TOTAL.get_or_init(|| {
        register_counter_vec!(
            "stellar_cross_anchor_transfers_total",
            "Total cross-anchor SEP-31 transfers by status",
            &["anchor_domain", "status"]
        )
        .expect("register stellar_cross_anchor_transfers_total")
    })
}

#[cfg(feature = "database")]
fn anchor_trustline_reserve_low() -> &'static GaugeVec {
    ANCHOR_TRUSTLINE_RESERVE_LOW.get_or_init(|| {
        register_gauge_vec!(
            "stellar_anchor_trustline_reserve_low",
            "1 if a cNGN or peer stablecoin trustline is below safe base reserve, 0 otherwise",
            &["asset", "account"]
        )
        .expect("register stellar_anchor_trustline_reserve_low")
    })
}

#[cfg(feature = "database")]
fn pathfinding_failures_consecutive() -> &'static GaugeVec {
    PATHFINDING_FAILURES_CONSECUTIVE.get_or_init(|| {
        register_gauge_vec!(
            "stellar_pathfinding_consecutive_failures",
            "Consecutive pathfinding query failures (resets to 0 on success)",
            &["base_asset", "counter_asset"]
        )
        .expect("register stellar_pathfinding_consecutive_failures")
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Public recording functions
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
pub fn record_slippage_rejection(base_asset: &str, counter_asset: &str) {
    dex_slippage_rejections()
        .with_label_values(&[base_asset, counter_asset])
        .inc();
}

#[cfg(feature = "database")]
pub fn observe_pathfinding_duration(base_asset: &str, counter_asset: &str, secs: f64) {
    pathfinding_duration()
        .with_label_values(&[base_asset, counter_asset])
        .observe(secs);
}

#[cfg(feature = "database")]
pub fn observe_sep_api_latency(
    anchor_domain: &str,
    sep_protocol: &str,
    operation: &str,
    secs: f64,
) {
    anchor_sep_api_latency()
        .with_label_values(&[anchor_domain, sep_protocol, operation])
        .observe(secs);
}

#[cfg(feature = "database")]
pub fn record_transfer_outcome(anchor_domain: &str, status: &str) {
    cross_anchor_transfers_total()
        .with_label_values(&[anchor_domain, status])
        .inc();
}

#[cfg(feature = "database")]
pub fn set_trustline_reserve_low(asset: &str, account: &str, is_low: bool) {
    anchor_trustline_reserve_low()
        .with_label_values(&[asset, account])
        .set(if is_low { 1.0 } else { 0.0 });
}

#[cfg(feature = "database")]
pub fn set_consecutive_pathfinding_failures(base_asset: &str, counter_asset: &str, count: f64) {
    pathfinding_failures_consecutive()
        .with_label_values(&[base_asset, counter_asset])
        .set(count);
}
