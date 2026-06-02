//! Deterministic Cryptographic Merkle Tree Construction Engine
//!
//! Provides parallel tree building, proof generation, and verification logic
//! for multi-tenant balances.

use sha2::{Digest, Sha256};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use bigdecimal::BigDecimal;

/// Represents a single white-label tenant/partner balance record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantBalance {
    pub tenant_id: String,
    pub balance: BigDecimal,
}

/// A node in the Merkle path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MerklePathNode {
    /// Hex-encoded hash of the sibling node.
    pub hash: String,
    /// True if the sibling is on the left; false if on the right.
    pub is_left: bool,
}

/// A complete inclusion proof for a leaf in the Merkle tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MerkleProof {
    /// Hex-encoded hash of the target leaf.
    pub leaf: String,
    /// Hex-encoded root hash of the Merkle Tree.
    pub root: String,
    /// Index of the leaf in the tree.
    pub index: usize,
    /// List of sibling hashes and their positions traversing up to the root.
    pub path: Vec<MerklePathNode>,
}

/// Represents the constructed Merkle Tree.
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// Leaf hashes of the tree.
    pub leaves: Vec<[u8; 32]>,
    /// Levels of the tree from bottom (leaves) to top (root).
    pub levels: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    /// Construct a Merkle Tree deterministically from a list of tenant balances.
    /// Uses rayon parallel iterators to optimize leaf hashing and parent node generation.
    pub fn build(balances: &[TenantBalance]) -> Self {
        if balances.is_empty() {
            return MerkleTree {
                leaves: Vec::new(),
                levels: vec![vec![[0u8; 32]]],
            };
        }

        // 1. Hash leaves: H(Tenant ID || Balance)
        let leaves: Vec<[u8; 32]> = balances
            .par_iter()
            .map(|tb| {
                let mut hasher = Sha256::new();
                hasher.update(tb.tenant_id.as_bytes());
                hasher.update(tb.balance.to_string().as_bytes());
                let result = hasher.finalize();
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&result);
                arr
            })
            .collect();

        let mut levels = vec![leaves.clone()];
        let mut current_level = leaves.clone();

        // 2. Build tree upwards level-by-level
        while current_level.len() > 1 {
            let next_level: Vec<[u8; 32]> = current_level
                .par_chunks(2)
                .map(|chunk| {
                    let mut hasher = Sha256::new();
                    if chunk.len() == 2 {
                        // Concatenate left and right siblings
                        hasher.update(&chunk[0]);
                        hasher.update(&chunk[1]);
                    } else {
                        // Odd node: duplicate the single node
                        hasher.update(&chunk[0]);
                        hasher.update(&chunk[0]);
                    }
                    let result = hasher.finalize();
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&result);
                    arr
                })
                .collect();

            levels.push(next_level.clone());
            current_level = next_level;
        }

        MerkleTree { leaves, levels }
    }

    /// Returns the root hash of the tree.
    pub fn root(&self) -> [u8; 32] {
        self.levels
            .last()
            .and_then(|lvl| lvl.first())
            .cloned()
            .unwrap_or([0u8; 32])
    }

    /// Returns the hex-encoded root hash of the tree.
    pub fn root_hex(&self) -> String {
        hex::encode(self.root())
    }

    /// Returns the depth of the Merkle Tree.
    pub fn tree_depth(&self) -> usize {
        if self.levels.is_empty() {
            0
        } else {
            self.levels.len() - 1
        }
    }

    /// Generates a succinct Merkle proof for a leaf index.
    pub fn generate_proof(&self, index: usize) -> Option<MerkleProof> {
        if index >= self.leaves.len() {
            return None;
        }

        let mut path = Vec::new();
        let mut idx = index;

        // Traverse from bottom level (leaves) up to, but not including, the root level.
        for level_idx in 0..(self.levels.len() - 1) {
            let level = &self.levels[level_idx];
            let is_right = idx % 2 == 1;
            let sibling_idx = if is_right { idx - 1 } else { idx + 1 };

            let sibling_hash = if sibling_idx < level.len() {
                level[sibling_idx]
            } else {
                level[idx] // Duplicate node if odd
            };

            path.push(MerklePathNode {
                hash: hex::encode(sibling_hash),
                is_left: is_right,
            });

            idx /= 2;
        }

        Some(MerkleProof {
            leaf: hex::encode(self.leaves[index]),
            root: self.root_hex(),
            index,
            path,
        })
    }

    /// Verifies a client-supplied MerkleProof witness path.
    pub fn verify_proof(proof: &MerkleProof) -> bool {
        let leaf_bytes = match hex::decode(&proof.leaf) {
            Ok(bytes) => {
                if bytes.len() != 32 {
                    return false;
                }
                bytes
            }
            Err(_) => return false,
        };

        let root_bytes = match hex::decode(&proof.root) {
            Ok(bytes) => {
                if bytes.len() != 32 {
                    return false;
                }
                bytes
            }
            Err(_) => return false,
        };

        let mut current_hash = [0u8; 32];
        current_hash.copy_from_slice(&leaf_bytes);

        for node in &proof.path {
            let sibling = match hex::decode(&node.hash) {
                Ok(bytes) => {
                    if bytes.len() != 32 {
                        return false;
                    }
                    bytes
                }
                Err(_) => return false,
            };

            let mut hasher = Sha256::new();
            if node.is_left {
                hasher.update(&sibling);
                hasher.update(&current_hash);
            } else {
                hasher.update(&current_hash);
                hasher.update(&sibling);
            }
            let result = hasher.finalize();
            current_hash.copy_from_slice(&result);
        }

        current_hash == root_bytes.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_merkle_tree_empty() {
        let tree = MerkleTree::build(&[]);
        assert_eq!(tree.root(), [0u8; 32]);
        assert_eq!(tree.tree_depth(), 0);
    }

    #[test]
    fn test_merkle_tree_single() {
        let balances = vec![TenantBalance {
            tenant_id: "tenant-1".to_string(),
            balance: BigDecimal::from_str("150.50").unwrap(),
        }];
        let tree = MerkleTree::build(&balances);
        assert_eq!(tree.tree_depth(), 0);
        assert_eq!(tree.leaves.len(), 1);
        assert_eq!(tree.root(), tree.leaves[0]);
    }

    #[test]
    fn test_merkle_tree_multiple() {
        let balances = vec![
            TenantBalance {
                tenant_id: "tenant-1".to_string(),
                balance: BigDecimal::from_str("100").unwrap(),
            },
            TenantBalance {
                tenant_id: "tenant-2".to_string(),
                balance: BigDecimal::from_str("200").unwrap(),
            },
            TenantBalance {
                tenant_id: "tenant-3".to_string(),
                balance: BigDecimal::from_str("300").unwrap(),
            },
        ];

        let tree = MerkleTree::build(&balances);
        assert_eq!(tree.leaves.len(), 3);
        assert_eq!(tree.tree_depth(), 2); // leaves -> level 1 (2 nodes) -> level 2 (root, 1 node)

        // Check root matches expected
        let root = tree.root();
        assert_ne!(root, [0u8; 32]);

        // Generate and verify proofs
        for i in 0..3 {
            let proof = tree.generate_proof(i).unwrap();
            assert!(MerkleTree::verify_proof(&proof));
            
            // Tamper with index/path and verify it fails
            let mut tampered_proof = proof.clone();
            tampered_proof.leaf = hex::encode([0u8; 32]);
            assert!(!MerkleTree::verify_proof(&tampered_proof));
        }
    }
}
