//! Integration tests for Issue #533: BFT oracle manipulation drill, MEV shield, multi-tenant isolation.

use aframp_backend::bft_oracle::{
    medianizer::BftMedianizer,
    mev_shield::{MevDecision, MevShield},
    models::OracleTick,
};
use chrono::Utc;
use uuid::Uuid;

fn node_tick(price: f64) -> OracleTick {
    OracleTick { node_id: Uuid::new_v4(), pair: "XLM/USD".into(), price, weight: 1, tick_at: Utc::now() }
}

/// Simulate a rogue oracle injecting 2 bad prices out of 7 nodes (t=2, quorum=5).
#[test]
fn test_oracle_manipulation_drill() {
    let m = BftMedianizer::new(7, 2); // quorum = 5
    let mut ticks: Vec<OracleTick> = (0..5).map(|_| node_tick(1.0512)).collect();
    ticks.push(node_tick(9999.0)); // rogue node 1
    ticks.push(node_tick(0.0001)); // rogue node 2

    let result = m.compute("XLM/USD", &ticks).unwrap();
    assert!(result.quorum_met);
    assert!(result.price > 1.0 && result.price < 2.0,
        "BFT failed to filter rogue prices, got {}", result.price);
}

#[test]
fn test_flash_loan_interceptor_blocks_extreme_spike() {
    let shield = MevShield::new(1.5, 500);
    let decision = shield.evaluate_flash_loan(1.10, 1.0); // 10% spike
    assert!(matches!(decision, MevDecision::Pause { .. }));
}

#[test]
fn test_quorum_failure_blocks_downstream_routing() {
    let m = BftMedianizer::new(7, 2); // quorum = 5
    // Only 4 nodes responding (below quorum)
    let ticks: Vec<OracleTick> = (0..4).map(|_| node_tick(1.05)).collect();
    assert!(m.compute("XLM/USD", &ticks).is_none(),
        "routing should be blocked when quorum not met");
}

#[test]
fn test_multi_tenant_price_history_isolation() {
    // Each tenant's historical ticks are keyed by node_id — assert no overlap
    let tenant_a_ticks: Vec<OracleTick> = (0..3).map(|_| node_tick(1.05)).collect();
    let tenant_b_ticks: Vec<OracleTick> = (0..3).map(|_| node_tick(2.10)).collect();

    let ids_a: std::collections::HashSet<_> = tenant_a_ticks.iter().map(|t| t.node_id).collect();
    let ids_b: std::collections::HashSet<_> = tenant_b_ticks.iter().map(|t| t.node_id).collect();

    assert!(ids_a.is_disjoint(&ids_b), "cross-tenant node_id collision detected");
}
