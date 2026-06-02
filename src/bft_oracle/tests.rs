//! Integration-level unit tests for Issue #533: BFT medianizer, Winsorized filter, MEV detection.

use uuid::Uuid;
use chrono::Utc;
use crate::bft_oracle::{
    medianizer::BftMedianizer,
    mev_shield::{MevDecision, MevShield},
    models::OracleTick,
};

fn t(price: f64) -> OracleTick {
    OracleTick { node_id: Uuid::new_v4(), pair: "NGN/USD".into(), price, weight: 1, tick_at: Utc::now() }
}

#[test]
fn test_bft_stable_under_single_byzantine_node() {
    // 5 nodes, 1 Byzantine (quorum = 3)
    let m = BftMedianizer::new(5, 1);
    let ticks = vec![t(0.0012), t(0.0012), t(0.0013), t(0.0012), t(500.0)]; // 500.0 is rogue
    let price = m.compute("NGN/USD", &ticks).unwrap();
    assert!(price < 1.0, "Byzantine price leaked: {}", price);
}

#[test]
fn test_mev_shield_blocks_flash_loan() {
    let shield = MevShield::new(1.5, 200);
    let decision = shield.evaluate_flash_loan(0.0020, 0.0012); // ~66% spike
    assert!(matches!(decision, MevDecision::Pause { .. }));
}

#[test]
fn test_bft_quorum_boundary_exact() {
    let m = BftMedianizer::new(5, 2); // quorum = 5
    // Exactly quorum ticks
    let ticks: Vec<OracleTick> = (0..5).map(|_| t(1.05)).collect();
    assert!(m.compute("XLM/USD", &ticks).is_some());
    // One less than quorum
    let ticks_short: Vec<OracleTick> = (0..4).map(|_| t(1.05)).collect();
    assert!(m.compute("XLM/USD", &ticks_short).is_none());
}
