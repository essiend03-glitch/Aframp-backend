//! Prometheus metrics for the Risk Management module — Issue #494.

use lazy_static::lazy_static;
use prometheus::{GaugeVec, IntCounterVec, Opts, Registry};

lazy_static! {
    static ref CORRIDOR_RISK_SCORE: GaugeVec = GaugeVec::new(
        Opts::new("corridor_risk_score", "Current risk score per corridor (0-100)"),
        &["corridor"]
    )
    .unwrap();

    static ref CIRCUIT_BREAKER_TRIPS: IntCounterVec = IntCounterVec::new(
        Opts::new("circuit_breaker_trips_total", "Total circuit breaker trips"),
        &["corridor", "trigger"]
    )
    .unwrap();

    static ref BANK_API_LATENCY: GaugeVec = GaugeVec::new(
        Opts::new("bank_api_latency_seconds", "Bank API latency in seconds"),
        &["bank_id"]
    )
    .unwrap();

    static ref VOLATILITY_SIGMA: GaugeVec = GaugeVec::new(
        Opts::new("volatility_deviation_sigma", "Current volatility deviation in sigma units"),
        &["pair"]
    )
    .unwrap();
}

pub fn register(registry: &Registry) {
    let _ = registry.register(Box::new(CORRIDOR_RISK_SCORE.clone()));
    let _ = registry.register(Box::new(CIRCUIT_BREAKER_TRIPS.clone()));
    let _ = registry.register(Box::new(BANK_API_LATENCY.clone()));
    let _ = registry.register(Box::new(VOLATILITY_SIGMA.clone()));
}

pub fn corridor_risk_score() -> &'static GaugeVec { &CORRIDOR_RISK_SCORE }
pub fn circuit_breaker_trips() -> &'static IntCounterVec { &CIRCUIT_BREAKER_TRIPS }
pub fn bank_api_latency() -> &'static GaugeVec { &BANK_API_LATENCY }
pub fn volatility_sigma() -> &'static GaugeVec { &VOLATILITY_SIGMA }
