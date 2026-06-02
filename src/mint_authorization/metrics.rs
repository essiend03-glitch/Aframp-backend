//! Prometheus metrics for the Mint Authorization Framework.

use prometheus::{
    register_counter_with_registry, register_gauge_with_registry, Counter, Gauge,
};
use std::sync::OnceLock;

use crate::metrics::registry;

// ─────────────────────────────────────────────────────────────────────────────
// Counters
// ─────────────────────────────────────────────────────────────────────────────

static REQUESTS_CREATED: OnceLock<Counter> = OnceLock::new();
static SIGNATURES_COLLECTED: OnceLock<Counter> = OnceLock::new();
static THRESHOLDS_MET: OnceLock<Counter> = OnceLock::new();
static SUBMISSIONS_ATTEMPTED: OnceLock<Counter> = OnceLock::new();
static CONFIRMATIONS_RECEIVED: OnceLock<Counter> = OnceLock::new();
static FAILURES: OnceLock<Counter> = OnceLock::new();
static EXPIRATIONS: OnceLock<Counter> = OnceLock::new();
static CANCELLATIONS: OnceLock<Counter> = OnceLock::new();

// ─────────────────────────────────────────────────────────────────────────────
// Gauges
// ─────────────────────────────────────────────────────────────────────────────

static PENDING_COUNT: OnceLock<Gauge> = OnceLock::new();

// ─────────────────────────────────────────────────────────────────────────────
// Registration
// ─────────────────────────────────────────────────────────────────────────────

pub fn register(r: &prometheus::Registry) {
    macro_rules! reg_counter {
        ($cell:expr, $name:expr, $help:expr) => {
            $cell.get_or_init(|| {
                register_counter_with_registry!($name, $help, r).expect(concat!("register ", $name))
            });
        };
    }
    macro_rules! reg_gauge {
        ($cell:expr, $name:expr, $help:expr) => {
            $cell.get_or_init(|| {
                register_gauge_with_registry!($name, $help, r).expect(concat!("register ", $name))
            });
        };
    }

    reg_counter!(REQUESTS_CREATED,     "aframp_mint_auth_requests_created_total",     "Total mint authorization requests created");
    reg_counter!(SIGNATURES_COLLECTED, "aframp_mint_auth_signatures_collected_total", "Total signatures collected across all requests");
    reg_counter!(THRESHOLDS_MET,       "aframp_mint_auth_thresholds_met_total",       "Total requests that reached signature threshold");
    reg_counter!(SUBMISSIONS_ATTEMPTED,"aframp_mint_auth_submissions_attempted_total","Total Stellar submission attempts");
    reg_counter!(CONFIRMATIONS_RECEIVED,"aframp_mint_auth_confirmations_received_total","Total on-chain confirmations received");
    reg_counter!(FAILURES,             "aframp_mint_auth_failures_total",             "Total authorization request failures");
    reg_counter!(EXPIRATIONS,          "aframp_mint_auth_expirations_total",          "Total authorization requests expired");
    reg_counter!(CANCELLATIONS,        "aframp_mint_auth_cancellations_total",        "Total authorization requests cancelled");
    reg_gauge!(PENDING_COUNT,          "aframp_mint_auth_pending_count",              "Current number of pending-signatures authorization requests");
}

// ─────────────────────────────────────────────────────────────────────────────
// Accessors (initialise lazily via global registry if not pre-registered)
// ─────────────────────────────────────────────────────────────────────────────

fn ensure_registered() {
    if REQUESTS_CREATED.get().is_none() {
        register(registry());
    }
}

pub fn inc_requests_created() {
    ensure_registered();
    if let Some(c) = REQUESTS_CREATED.get() { c.inc(); }
}

pub fn inc_signatures_collected() {
    ensure_registered();
    if let Some(c) = SIGNATURES_COLLECTED.get() { c.inc(); }
}

pub fn inc_thresholds_met() {
    ensure_registered();
    if let Some(c) = THRESHOLDS_MET.get() { c.inc(); }
}

pub fn inc_submissions_attempted() {
    ensure_registered();
    if let Some(c) = SUBMISSIONS_ATTEMPTED.get() { c.inc(); }
}

pub fn inc_confirmations_received() {
    ensure_registered();
    if let Some(c) = CONFIRMATIONS_RECEIVED.get() { c.inc(); }
}

pub fn inc_failures() {
    ensure_registered();
    if let Some(c) = FAILURES.get() { c.inc(); }
}

pub fn inc_expirations() {
    ensure_registered();
    if let Some(c) = EXPIRATIONS.get() { c.inc(); }
}

pub fn inc_cancellations() {
    ensure_registered();
    if let Some(c) = CANCELLATIONS.get() { c.inc(); }
}

pub fn set_pending_count(n: f64) {
    ensure_registered();
    if let Some(g) = PENDING_COUNT.get() { g.set(n); }
}
