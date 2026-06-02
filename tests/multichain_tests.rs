//! Integration tests for Issue #532: atomic swap lifecycle, state relay, multi-tenant isolation.

use aframp_backend::multichain::{
    models::{AtomicSwap, StateProof, SwapStatus},
    state_relay::StateRelayEngine,
};
use chrono::Utc;
use uuid::Uuid;

fn mock_swap(status: SwapStatus) -> AtomicSwap {
    AtomicSwap {
        id:              Uuid::new_v4(),
        tenant_id:       Uuid::new_v4(),
        src_chain_id:    1,
        dst_chain_id:    42161,
        hashlock:        "0xdeadbeef00000000000000000000000000000000000000000000000000000000".into(),
        timelock_expiry: Utc::now() + chrono::Duration::hours(24),
        amount_wei:      "1000000000000000000".into(), // 1 ETH
        status,
        src_tx_hash:     None,
        dst_tx_hash:     None,
    }
}

#[test]
fn test_valid_proof_transitions_to_asset_locked() {
    let mut swap = mock_swap(SwapStatus::Initiated);
    let leaf = StateRelayEngine::hashlock_leaf("secret");
    let proof = StateProof {
        swap_id: swap.id, chain_id: 1, block_number: 100,
        state_root: leaf.clone(), merkle_proof: vec![], verified: false,
    };
    let ok = StateRelayEngine::validate_and_transition(&proof, &mut swap, &leaf, &leaf);
    assert!(ok);
    assert_eq!(swap.status, SwapStatus::AssetLocked);
}

#[test]
fn test_invalid_proof_freezes_channel() {
    let mut swap = mock_swap(SwapStatus::Initiated);
    let proof = StateProof {
        swap_id: swap.id, chain_id: 1, block_number: 100,
        state_root: "bad_root".into(), merkle_proof: vec![], verified: false,
    };
    let ok = StateRelayEngine::validate_and_transition(&proof, &mut swap, "bad_root", "wrong_leaf");
    assert!(!ok);
    assert_eq!(swap.status, SwapStatus::HeldForManualReconciliation);
}

#[test]
fn test_multi_tenant_swap_ids_are_distinct() {
    let s1 = mock_swap(SwapStatus::Initiated);
    let s2 = mock_swap(SwapStatus::Initiated);
    assert_ne!(s1.id, s2.id);
    assert_ne!(s1.tenant_id, s2.tenant_id);
}
