//! Prometheus metrics for Issue #532.

use lazy_static::lazy_static;
use prometheus::{register_counter_vec, register_gauge_vec, register_histogram_vec,
    CounterVec, GaugeVec, HistogramVec};

lazy_static! {
    pub static ref SWAP_LATENCY: HistogramVec = register_histogram_vec!(
        "cross_chain_swap_latency_seconds",
        "End-to-end atomic swap completion latency",
        &["src_chain", "dst_chain"],
        vec![1.0, 5.0, 15.0, 30.0, 60.0, 120.0, 300.0]
    ).unwrap();

    pub static ref GAS_ESCALATION_BURN: CounterVec = register_counter_vec!(
        "gas_escalation_burn_native",
        "Cumulative native gas burned in fee escalation bumps",
        &["chain_id"]
    ).unwrap();

    pub static ref RELAY_VERIFICATION_FAILURES: CounterVec = register_counter_vec!(
        "state_relay_verification_failures",
        "State proof verification failures per chain",
        &["chain_id"]
    ).unwrap();

    pub static ref ESCROW_ACTIVE_TVL_USD: GaugeVec = register_gauge_vec!(
        "escrow_active_tvl_usd",
        "Total value locked in active escrows in USD equivalent",
        &["dst_chain"]
    ).unwrap();
}
