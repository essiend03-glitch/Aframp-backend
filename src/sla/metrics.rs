//! Prometheus metrics for the SLA module — Issue #464.

use lazy_static::lazy_static;
use prometheus::{GaugeVec, IntCounterVec, Opts, Registry};

lazy_static! {
    static ref COMPLIANCE_RATIO: prometheus::Gauge = prometheus::Gauge::new(
        "sla_compliance_ratio_percentage",
        "Rolling SLA compliance ratio across all active corridors (0-100)"
    )
    .unwrap();

    static ref CURRENT_LATENCY: GaugeVec = GaugeVec::new(
        Opts::new("sla_current_latency_seconds", "Current P95/P99 latency per corridor"),
        &["corridor", "percentile"]
    )
    .unwrap();

    static ref ACTIVE_BREACHES: IntCounterVec = IntCounterVec::new(
        Opts::new("sla_active_breaches_count", "Total SLA breach events per corridor"),
        &["corridor"]
    )
    .unwrap();
}

pub fn register(registry: &Registry) {
    let _ = registry.register(Box::new(COMPLIANCE_RATIO.clone()));
    let _ = registry.register(Box::new(CURRENT_LATENCY.clone()));
    let _ = registry.register(Box::new(ACTIVE_BREACHES.clone()));
}

pub fn compliance_ratio() -> &'static prometheus::Gauge {
    &COMPLIANCE_RATIO
}

pub fn current_latency() -> &'static GaugeVec {
    &CURRENT_LATENCY
}

pub fn active_breaches() -> &'static IntCounterVec {
    &ACTIVE_BREACHES
}

/// Snapshot of current active breach count (sum across all corridors).
pub fn active_breaches_count() -> f64 {
    ACTIVE_BREACHES
        .collect()
        .into_iter()
        .flat_map(|mf| mf.get_metric().to_vec())
        .map(|m| m.get_counter().get_value())
        .sum()
}
