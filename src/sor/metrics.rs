//! #487 Smart Order Routing — Prometheus metrics.

use prometheus::{
    register_counter, register_gauge, register_histogram, Counter, Gauge, Histogram,
};
use std::sync::OnceLock;

static ORDERS_ROUTED: OnceLock<Counter> = OnceLock::new();
static ROUTING_FAILURES: OnceLock<Counter> = OnceLock::new();
static SLIPPAGE_BREACHES: OnceLock<Counter> = OnceLock::new();
static ROLLBACKS: OnceLock<Counter> = OnceLock::new();
static REBALANCE_FAILURES: OnceLock<Counter> = OnceLock::new();
static ROUTING_DURATION: OnceLock<Histogram> = OnceLock::new();
static SLIPPAGE_SAVED: OnceLock<Histogram> = OnceLock::new();
static REBALANCE_VOLUME: OnceLock<Histogram> = OnceLock::new();
static ACTIVE_VENUES: OnceLock<Gauge> = OnceLock::new();

pub fn orders_routed() -> &'static Counter {
    ORDERS_ROUTED.get_or_init(|| {
        register_counter!(
            "aframp_sor_orders_routed_total",
            "Total smart orders successfully routed"
        )
        .expect("register sor orders_routed")
    })
}

pub fn routing_failures() -> &'static Counter {
    ROUTING_FAILURES.get_or_init(|| {
        register_counter!(
            "aframp_sor_routing_failures_total",
            "Total SOR routing failures"
        )
        .expect("register sor routing_failures")
    })
}

pub fn slippage_breaches() -> &'static Counter {
    SLIPPAGE_BREACHES.get_or_init(|| {
        register_counter!(
            "aframp_sor_slippage_breaches_total",
            "Orders halted due to slippage limit breach"
        )
        .expect("register sor slippage_breaches")
    })
}

pub fn rollbacks() -> &'static Counter {
    ROLLBACKS.get_or_init(|| {
        register_counter!(
            "aframp_sor_rollbacks_total",
            "Execution rollbacks due to venue timeout or error"
        )
        .expect("register sor rollbacks")
    })
}

pub fn rebalance_failures() -> &'static Counter {
    REBALANCE_FAILURES.get_or_init(|| {
        register_counter!(
            "aframp_sor_rebalance_failures_total",
            "P1: automated rebalancing failures"
        )
        .expect("register sor rebalance_failures")
    })
}

pub fn routing_duration() -> &'static Histogram {
    ROUTING_DURATION.get_or_init(|| {
        register_histogram!(
            "aframp_sor_routing_duration_seconds",
            "SOR path calculation duration in seconds",
            vec![0.005, 0.010, 0.020, 0.040, 0.100, 0.250]
        )
        .expect("register sor routing_duration")
    })
}

pub fn slippage_saved() -> &'static Histogram {
    SLIPPAGE_SAVED.get_or_init(|| {
        register_histogram!(
            "aframp_sor_order_slippage_bps_saved",
            "Basis points saved vs max slippage limit per order",
            vec![0.0, 1.0, 5.0, 10.0, 25.0, 50.0]
        )
        .expect("register sor slippage_saved")
    })
}

pub fn rebalance_volume() -> &'static Histogram {
    REBALANCE_VOLUME.get_or_init(|| {
        register_histogram!(
            "aframp_sor_automated_rebalance_volume_total",
            "Volume rebalanced per operation in USD",
            vec![1000.0, 10000.0, 50000.0, 100000.0, 500000.0]
        )
        .expect("register sor rebalance_volume")
    })
}

pub fn active_venues() -> &'static Gauge {
    ACTIVE_VENUES.get_or_init(|| {
        register_gauge!(
            "aframp_sor_liquidity_venue_active_count",
            "Number of active liquidity venues"
        )
        .expect("register sor active_venues")
    })
}
