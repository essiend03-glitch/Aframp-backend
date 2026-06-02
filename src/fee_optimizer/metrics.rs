//! #490 Gas & Fee Optimization — Prometheus metrics.

use prometheus::{
    register_counter, register_histogram, Counter, Histogram,
};
use std::sync::OnceLock;

static GAS_SPENT: OnceLock<Counter> = OnceLock::new();
static BUMP_EVENTS: OnceLock<Counter> = OnceLock::new();
static CONGESTION_HALTS: OnceLock<Counter> = OnceLock::new();
static RPC_FEE_LATENCY: OnceLock<Histogram> = OnceLock::new();
static FEE_OPTIMIZATION_SAVINGS: OnceLock<Histogram> = OnceLock::new();

pub fn gas_spent() -> &'static Counter {
    GAS_SPENT.get_or_init(|| {
        register_counter!(
            "aframp_fee_gas_spent_native_total",
            "Total gas/fees spent in native sub-units across all chains"
        )
        .expect("register fee gas_spent")
    })
}

pub fn bump_events() -> &'static Counter {
    BUMP_EVENTS.get_or_init(|| {
        register_counter!(
            "aframp_fee_transaction_bump_events_total",
            "Total fee-bump replacement transactions issued"
        )
        .expect("register fee bump_events")
    })
}

pub fn congestion_halts() -> &'static Counter {
    CONGESTION_HALTS.get_or_init(|| {
        register_counter!(
            "aframp_fee_congestion_halts_total",
            "Batch payouts halted due to fee congestion threshold"
        )
        .expect("register fee congestion_halts")
    })
}

pub fn rpc_fee_latency() -> &'static Histogram {
    RPC_FEE_LATENCY.get_or_init(|| {
        register_histogram!(
            "aframp_fee_rpc_fee_latency_seconds",
            "RPC fee telemetry fetch latency in seconds",
            vec![0.001, 0.005, 0.010, 0.015, 0.050, 0.100]
        )
        .expect("register fee rpc_fee_latency")
    })
}

pub fn fee_optimization_savings() -> &'static Histogram {
    FEE_OPTIMIZATION_SAVINGS.get_or_init(|| {
        register_histogram!(
            "aframp_fee_optimization_savings_bps",
            "Fee savings in native sub-units vs max cap per transaction",
            vec![0.0, 100.0, 1000.0, 10000.0, 100000.0]
        )
        .expect("register fee fee_optimization_savings")
    })
}
