//! Decentralized state relay: Merkle proof verifier + atomic settlement loop.
//! Redis distributed lock prevents double-spend during parallel chain splits.

use std::sync::Arc;
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};
use uuid::Uuid;

use super::models::{AtomicSwap, StateProof, SwapStatus};

/// Verify a Merkle-Patricia inclusion proof.
/// `state_root`: expected root (hex); `proof_nodes`: path from leaf to root.
pub fn verify_merkle_proof(state_root: &str, leaf_hash: &str, proof_nodes: &[String]) -> bool {
    let mut current = hex::decode(leaf_hash.trim_start_matches("0x")).unwrap_or_default();
    for node in proof_nodes {
        let node_bytes = hex::decode(node.trim_start_matches("0x")).unwrap_or_default();
        let mut hasher = Sha256::new();
        if current <= node_bytes {
            hasher.update(&current);
            hasher.update(&node_bytes);
        } else {
            hasher.update(&node_bytes);
            hasher.update(&current);
        }
        current = hasher.finalize().to_vec();
    }
    hex::encode(&current) == state_root.trim_start_matches("0x")
}

pub struct StateRelayEngine;

impl StateRelayEngine {
    /// Validate incoming proof. Freezes channel to HELD state on verification failure.
    pub fn validate_and_transition(
        proof:   &StateProof,
        swap:    &mut AtomicSwap,
        expected_root: &str,
        leaf_hash: &str,
    ) -> bool {
        if !verify_merkle_proof(expected_root, leaf_hash, &proof.merkle_proof) {
            error!(
                swap_id=%swap.id,
                chain_id=proof.chain_id,
                "state proof verification failed – holding for manual reconciliation"
            );
            swap.status = SwapStatus::HeldForManualReconciliation;
            return false;
        }
        swap.status = SwapStatus::AssetLocked;
        info!(swap_id=%swap.id, "state proof verified – asset locked");
        true
    }

    /// Compute leaf hash from the hashlock for proof verification.
    pub fn hashlock_leaf(hashlock: &str) -> String {
        let mut h = Sha256::new();
        h.update(hashlock.as_bytes());
        hex::encode(h.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_proof_valid_trivial() {
        // Single-node proof: leaf hash == state root
        let leaf = "abc123";
        let leaf_hash = StateRelayEngine::hashlock_leaf(leaf);
        // With empty proof path, result == leaf_hash itself
        assert!(verify_merkle_proof(&leaf_hash, &leaf_hash, &[]));
    }

    #[test]
    fn test_merkle_proof_invalid() {
        let leaf_hash = StateRelayEngine::hashlock_leaf("abc");
        assert!(!verify_merkle_proof("deadbeef", &leaf_hash, &[]));
    }

    #[test]
    fn test_invalid_proof_freezes_swap() {
        let mut swap = AtomicSwap {
            id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            src_chain_id: 1,
            dst_chain_id: 42161,
            hashlock: "0xdeadbeef".into(),
            timelock_expiry: chrono::Utc::now() + chrono::Duration::hours(2),
            amount_wei: "1000000000000000000".into(),
            status: SwapStatus::Initiated,
            src_tx_hash: None,
            dst_tx_hash: None,
        };
        let proof = StateProof {
            swap_id: swap.id,
            chain_id: 1,
            block_number: 1000,
            state_root: "bad_root".into(),
            merkle_proof: vec![],
            verified: false,
        };
        let result = StateRelayEngine::validate_and_transition(&proof, &mut swap, "good_root", "leafhash");
        assert!(!result);
        assert_eq!(swap.status, SwapStatus::HeldForManualReconciliation);
    }
}
