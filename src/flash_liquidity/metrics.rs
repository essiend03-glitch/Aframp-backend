//! #488 Flash Liquidity — Prometheus metrics.

use prometheus::{
    register_counter, register_gauge, register_histogram, Counter, Gauge, Histogram,
};
use std::sync::OnceLock;

static DRAWS_TOTAL: OnceLock<Counter> = OnceLock::new();
static REPAYMENTS_TOTAL: OnceLock<Counter> = OnceLock::new();
static REPAYMENT_FAILURES: OnceLock<Counter> = OnceLock::new();
static COLLATERAL_LOCKED: OnceLock<Counter> = OnceLock::new();
static LIQUIDATION_DEFENSE_ACTIONS: OnceLock<Counter> = OnceLock::new();
static CREDIT_UTILIZATION: OnceLock<Gauge> = OnceLock::new();
static HEALTH_FACTOR: OnceLock<Gauge> = OnceLock::new();
static INTEREST_ACCRUED: OnceLock<Histogram> = OnceLock::new();

pub fn draws_total() -> &'static Counter {
    DRAWS_TOTAL.get_or_init(|| {
        register_counter!(
            "aframp_flash_draws_total",
            "Total flash liquidity draws executed"
        )
        .expect("register flash draws_total")
    })
}

pub fn repayments_total() -> &'static Counter {
    REPAYMENTS_TOTAL.get_or_init(|| {
        register_counter!(
            "aframp_flash_repayments_total",
            "Total flash draws repaid"
        )
        .expect("register flash repayments_total")
    })
}

pub fn repayment_failures() -> &'static Counter {
    REPAYMENT_FAILURES.get_or_init(|| {
        register_counter!(
            "aframp_flash_repayment_failures_total",
            "P1: flash repayment failures"
        )
        .expect("register flash repayment_failures")
    })
}

pub fn collateral_locked() -> &'static Counter {
    COLLATERAL_LOCKED.get_or_init(|| {
        register_counter!(
            "aframp_flash_collateral_locked_total",
            "Total collateral lock operations"
        )
        .expect("register flash collateral_locked")
    })
}

pub fn liquidation_defense_actions() -> &'static Counter {
    LIQUIDATION_DEFENSE_ACTIONS.get_or_init(|| {
        register_counter!(
            "aframp_flash_liquidation_defense_actions_total",
            "Circuit breaker liquidation defense actions triggered"
        )
        .expect("register flash liquidation_defense_actions")
    })
}

pub fn credit_utilization() -> &'static Gauge {
    CREDIT_UTILIZATION.get_or_init(|| {
        register_gauge!(
            "aframp_flash_credit_utilization_ratio",
            "Current flash credit utilization in USD"
        )
        .expect("register flash credit_utilization")
    })
}

pub fn health_factor() -> &'static Gauge {
    HEALTH_FACTOR.get_or_init(|| {
        register_gauge!(
            "aframp_flash_collateral_health_factor",
            "Latest collateral health factor across active draws"
        )
        .expect("register flash health_factor")
    })
}

pub fn interest_accrued() -> &'static Histogram {
    INTEREST_ACCRUED.get_or_init(|| {
        register_histogram!(
            "aframp_flash_intra_day_interest_accrued_cngn",
            "Intra-day interest accrued per repaid draw in cNGN",
            vec![0.0, 10.0, 50.0, 100.0, 500.0, 1000.0]
        )
        .expect("register flash interest_accrued")
    })
}
