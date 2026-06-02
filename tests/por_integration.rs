//! Integration tests for Proof of Reserves (PoR) and Merkle Tree Engine

#[cfg(test)]
mod tests {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;
    use crate::security::{TenantBalance, MerkleTree, MerkleProof, MerklePathNode};

    #[test]
    fn test_merkle_tree_proof_generation_and_verification() {
        // Arrange
        let balances = vec![
            TenantBalance {
                tenant_id: "tenant-a".to_string(),
                balance: BigDecimal::from_str("1250000.50").unwrap(),
            },
            TenantBalance {
                tenant_id: "tenant-b".to_string(),
                balance: BigDecimal::from_str("750000.75").unwrap(),
            },
            TenantBalance {
                tenant_id: "tenant-c".to_string(),
                balance: BigDecimal::from_str("3200000.00").unwrap(),
            },
            TenantBalance {
                tenant_id: "tenant-d".to_string(),
                balance: BigDecimal::from_str("0.00").unwrap(),
            },
        ];

        // Act
        let tree = MerkleTree::build(&balances);
        assert_eq!(tree.leaves.len(), 4);
        assert_eq!(tree.tree_depth(), 2);

        // Assert proofs for all leaves
        for index in 0..4 {
            let proof = tree.generate_proof(index).expect("Proof should be generated");
            
            // Verify correct values in proof
            assert_eq!(proof.index, index);
            assert_eq!(proof.root, tree.root_hex());
            assert_eq!(proof.leaf, hex::encode(tree.leaves[index]));

            // Mathematically verify proof
            let is_valid = MerkleTree::verify_proof(&proof);
            assert!(is_valid, "Proof at index {} should be valid", index);
        }
    }

    #[test]
    fn test_merkle_tree_tampering_detection() {
        // Arrange
        let balances = vec![
            TenantBalance {
                tenant_id: "tenant-a".to_string(),
                balance: BigDecimal::from_str("1250000.50").unwrap(),
            },
            TenantBalance {
                tenant_id: "tenant-b".to_string(),
                balance: BigDecimal::from_str("750000.75").unwrap(),
            },
        ];
        let tree = MerkleTree::build(&balances);
        let mut proof = tree.generate_proof(0).unwrap();

        // Act & Assert: Modifying leaf should fail verification
        proof.leaf = hex::encode([0u8; 32]);
        assert!(!MerkleTree::verify_proof(&proof));

        // Act & Assert: Modifying root should fail verification
        let mut proof2 = tree.generate_proof(0).unwrap();
        proof2.root = hex::encode([1u8; 32]);
        assert!(!MerkleTree::verify_proof(&proof2));

        // Act & Assert: Modifying path node should fail verification
        let mut proof3 = tree.generate_proof(0).unwrap();
        if !proof3.path.is_empty() {
            proof3.path[0].hash = hex::encode([2u8; 32]);
            assert!(!MerkleTree::verify_proof(&proof3));
        }
    }

    #[test]
    fn test_merkle_tree_odd_number_of_leaves() {
        // Arrange
        let balances = vec![
            TenantBalance {
                tenant_id: "tenant-a".to_string(),
                balance: BigDecimal::from_str("100.00").unwrap(),
            },
            TenantBalance {
                tenant_id: "tenant-b".to_string(),
                balance: BigDecimal::from_str("200.00").unwrap(),
            },
            TenantBalance {
                tenant_id: "tenant-c".to_string(),
                balance: BigDecimal::from_str("300.00").unwrap(),
            },
        ];

        // Act
        let tree = MerkleTree::build(&balances);

        // Assert: 3 leaves should succeed and verify
        assert_eq!(tree.leaves.len(), 3);
        
        for index in 0..3 {
            let proof = tree.generate_proof(index).unwrap();
            assert!(MerkleTree::verify_proof(&proof));
        }
    }
}
