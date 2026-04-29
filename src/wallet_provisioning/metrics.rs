//! Prometheus metrics for wallet provisioning (Issue #322).

use prometheus::{
    register_counter_vec, register_gauge_vec, CounterVec, GaugeVec,
};
use std::sync::OnceLock;

static PROVISIONING_INITIATIONS: OnceLock<CounterVec> = OnceLock::new();
static PROVISIONING_COMPLETIONS: OnceLock<CounterVec> = OnceLock::new();
static SPONSORSHIP_EVENTS: OnceLock<CounterVec> = OnceLock::new();
static TRUSTLINE_EVENTS: OnceLock<CounterVec> = OnceLock::new();
static PROVISIONING_ABANDONMENTS: OnceLock<CounterVec> = OnceLock::new();
static WALLETS_IN_STATE: OnceLock<GaugeVec> = OnceLock::new();
static FUNDING_ACCOUNT_BALANCE: OnceLock<GaugeVec> = OnceLock::new();

pub fn provisioning_initiations() -> &'static CounterVec {
    PROVISIONING_INITIATIONS.get_or_init(|| {
        register_counter_vec!(
            "wallet_provisioning_initiations_total",
            "Total wallet provisioning initiations",
            &["method"]
        )
        .expect("metric registration failed")
    })
}

pub fn provisioning_completions() -> &'static CounterVec {
    PROVISIONING_COMPLETIONS.get_or_init(|| {
        register_counter_vec!(
            "wallet_provisioning_completions_total",
            "Total wallet provisioning completions by method",
            &["method"]
        )
        .expect("metric registration failed")
    })
}

pub fn sponsorship_events() -> &'static CounterVec {
    SPONSORSHIP_EVENTS.get_or_init(|| {
        register_counter_vec!(
            "wallet_sponsorship_events_total",
            "Total platform-sponsored account creation events",
            &["outcome"]
        )
        .expect("metric registration failed")
    })
}

pub fn trustline_events() -> &'static CounterVec {
    TRUSTLINE_EVENTS.get_or_init(|| {
        register_counter_vec!(
            "wallet_trustline_events_total",
            "Trustline establishment events",
            &["outcome"]
        )
        .expect("metric registration failed")
    })
}

pub fn provisioning_abandonments() -> &'static CounterVec {
    PROVISIONING_ABANDONMENTS.get_or_init(|| {
        register_counter_vec!(
            "wallet_provisioning_abandonments_total",
            "Provisioning abandonments per step",
            &["step"]
        )
        .expect("metric registration failed")
    })
}

pub fn wallets_in_state() -> &'static GaugeVec {
    WALLETS_IN_STATE.get_or_init(|| {
        register_gauge_vec!(
            "wallet_provisioning_state_count",
            "Number of wallets in each provisioning state",
            &["state"]
        )
        .expect("metric registration failed")
    })
}

pub fn funding_account_balance() -> &'static GaugeVec {
    FUNDING_ACCOUNT_BALANCE.get_or_init(|| {
        register_gauge_vec!(
            "platform_funding_account_xlm_balance",
            "Current XLM balance of the platform funding account",
            &["address"]
        )
        .expect("metric registration failed")
    })
}
