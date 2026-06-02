//! Proof of Reserves (PoR) metrics.

use prometheus::{
    register_counter_vec_with_registry, register_gauge_vec_with_registry,
    register_histogram_vec_with_registry, CounterVec, GaugeVec, HistogramVec, Registry,
};
use std::sync::OnceLock;

static MERKLE_TREE_CONSTRUCTION_DURATION: OnceLock<HistogramVec> = OnceLock::new();
static RESERVE_BACKING_RATIO: OnceLock<GaugeVec> = OnceLock::new();
static PROOF_ANCHORING_FAILURES: OnceLock<CounterVec> = OnceLock::new();
static TOTAL_FIAT_RESERVES: OnceLock<GaugeVec> = OnceLock::new();

pub fn merkle_tree_construction_duration_seconds() -> &'static HistogramVec {
    MERKLE_TREE_CONSTRUCTION_DURATION
        .get()
        .expect("PoR metrics not initialised")
}

pub fn reserve_backing_ratio() -> &'static GaugeVec {
    RESERVE_BACKING_RATIO.get().expect("PoR metrics not initialised")
}

pub fn proof_anchoring_failures_total() -> &'static CounterVec {
    PROOF_ANCHORING_FAILURES
        .get()
        .expect("PoR metrics not initialised")
}

pub fn total_fiat_reserves_held() -> &'static GaugeVec {
    TOTAL_FIAT_RESERVES.get().expect("PoR metrics not initialised")
}

pub fn register(r: &Registry) {
    MERKLE_TREE_CONSTRUCTION_DURATION
        .set(
            register_histogram_vec_with_registry!(
                "aframp_por_merkle_tree_construction_duration_seconds",
                "Duration of Merkle Tree construction in seconds",
                &[],
                vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0],
                r
            )
            .unwrap(),
        )
        .ok();

    RESERVE_BACKING_RATIO
        .set(
            register_gauge_vec_with_registry!(
                "aframp_por_reserve_backing_ratio",
                "Ratio of aggregated fiat reserves to outstanding on-chain supply",
                &[],
                r
            )
            .unwrap(),
        )
        .ok();

    PROOF_ANCHORING_FAILURES
        .set(
            register_counter_vec_with_registry!(
                "aframp_por_proof_anchoring_failures_total",
                "Total number of failures during PoR proof calculation or anchoring",
                &[],
                r
            )
            .unwrap(),
        )
        .ok();

    TOTAL_FIAT_RESERVES
        .set(
            register_gauge_vec_with_registry!(
                "aframp_por_total_fiat_reserves_held",
                "Total off-chain fiat reserves held in NGN",
                &[],
                r
            )
            .unwrap(),
        )
        .ok();
}
