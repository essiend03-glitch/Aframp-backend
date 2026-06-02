//! Prometheus metrics for the database layer.
//!
//! Exposes:
//! - `aframp_replication_lag_seconds{replica}` — current replication lag per replica.
//! - `aframp_replication_circuit_breaker_open{replica}` — 1 when the circuit breaker is open.

use prometheus::{register_gauge_vec, GaugeVec};
use std::sync::OnceLock;

static REPLICATION_LAG: OnceLock<GaugeVec> = OnceLock::new();
static CIRCUIT_BREAKER_OPEN: OnceLock<GaugeVec> = OnceLock::new();

fn replication_lag() -> &'static GaugeVec {
    REPLICATION_LAG.get_or_init(|| {
        register_gauge_vec!(
            "aframp_replication_lag_seconds",
            "Current replication lag in seconds per read replica",
            &["replica"]
        )
        .expect("register aframp_replication_lag_seconds")
    })
}

fn circuit_breaker_open() -> &'static GaugeVec {
    CIRCUIT_BREAKER_OPEN.get_or_init(|| {
        register_gauge_vec!(
            "aframp_replication_circuit_breaker_open",
            "1 when the replication circuit breaker is open for a replica",
            &["replica"]
        )
        .expect("register aframp_replication_circuit_breaker_open")
    })
}

/// Update the replication lag gauge for a named replica.
pub fn set_replication_lag(replica: &str, lag_secs: f64) {
    replication_lag()
        .with_label_values(&[replica])
        .set(lag_secs);
}

/// Update the circuit breaker state gauge (1 = open, 0 = closed).
pub fn set_circuit_breaker(replica: &str, is_open: bool) {
    circuit_breaker_open()
        .with_label_values(&[replica])
        .set(if is_open { 1.0 } else { 0.0 });
}
