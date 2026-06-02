//! #491 Compliance Oracle — Prometheus metrics.

use prometheus::{
    register_counter, register_histogram, Counter, Histogram,
};
use std::sync::OnceLock;

static CACHE_HITS: OnceLock<Counter> = OnceLock::new();
static VERIFICATION_FAILURES: OnceLock<Counter> = OnceLock::new();
static ORACLE_QUERY_DURATION: OnceLock<Histogram> = OnceLock::new();
static ZK_PROOF_VALIDATION_LATENCY: OnceLock<Histogram> = OnceLock::new();

pub fn cache_hits() -> &'static Counter {
    CACHE_HITS.get_or_init(|| {
        register_counter!(
            "aframp_compliance_cached_compliance_hits_total",
            "Compliance verifications served from Redis cache"
        )
        .expect("register compliance cache_hits")
    })
}

pub fn verification_failures() -> &'static Counter {
    VERIFICATION_FAILURES.get_or_init(|| {
        register_counter!(
            "aframp_compliance_attestation_verification_failures_total",
            "Total attestation verification failures"
        )
        .expect("register compliance verification_failures")
    })
}

pub fn oracle_query_duration() -> &'static Histogram {
    ORACLE_QUERY_DURATION.get_or_init(|| {
        register_histogram!(
            "aframp_compliance_oracle_query_duration_seconds",
            "Oracle query round-trip duration in seconds",
            vec![0.010, 0.050, 0.100, 0.150, 0.300, 1.0]
        )
        .expect("register compliance oracle_query_duration")
    })
}

pub fn zk_proof_validation_latency() -> &'static Histogram {
    ZK_PROOF_VALIDATION_LATENCY.get_or_init(|| {
        register_histogram!(
            "aframp_compliance_zk_proof_validation_latency_seconds",
            "ZK proof validation latency in seconds",
            vec![0.001, 0.005, 0.010, 0.050, 0.100]
        )
        .expect("register compliance zk_proof_validation_latency")
    })
}
