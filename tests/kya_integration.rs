#[cfg(all(test, feature = "database"))]
mod kya_tests {
    use chrono::Utc;
    use sqlx::PgPool;
    use uuid::Uuid;

    use Bitmesh_backend::kya::{
        identity::AgentIdentity,
        models::{DID, ReputationDomain},
        registry::KYARegistry,
    };

    async fn setup_test_pool() -> PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/aframp_test".to_string());
        
        PgPool::connect(&database_url).await.expect("Failed to connect to test database")
    }

    #[tokio::test]
    async fn test_agent_registration() {
        let pool = setup_test_pool().await;
        let registry = KYARegistry::new(pool);

        let identity = AgentIdentity::new(
            "stellar",
            "testnet",
            "TestAgent".to_string(),
            "GTEST123".to_string(),
        )
        .expect("Failed to create identity");

        let result = registry.register_agent(&identity).await;
        assert!(result.is_ok(), "Agent registration failed");

        // Verify retrieval
        let retrieved = registry.get_agent(&identity.profile.did).await;
        assert!(retrieved.is_ok(), "Failed to retrieve agent");
        
        let retrieved_profile = retrieved.unwrap().export_profile();
        assert_eq!(retrieved_profile.name, "TestAgent");
    }

    #[tokio::test]
    async fn test_reputation_scoring() {
        let pool = setup_test_pool().await;
        let registry = KYARegistry::new(pool);

        let identity = AgentIdentity::new(
            "stellar",
            "testnet",
            "ReputationAgent".to_string(),
            "GREP123".to_string(),
        )
        .expect("Failed to create identity");

        registry.register_agent(&identity).await.expect("Registration failed");

        let domain = ReputationDomain::CodeAudit;

        // Initialize reputation
        registry
            .initialize_reputation(&identity.profile.did, &domain)
            .await
            .expect("Failed to initialize reputation");

        // Record successful interactions
        for _ in 0..10 {
            registry
                .record_interaction(&identity.profile.did, &domain, true, 1.0)
                .await
                .expect("Failed to record interaction");
        }

        // Record some failures
        for _ in 0..2 {
            registry
                .record_interaction(&identity.profile.did, &domain, false, 1.0)
                .await
                .expect("Failed to record interaction");
        }

        // Get reputation score
        let reputation = registry
            .get_reputation(&identity.profile.did, &domain)
            .await
            .expect("Failed to get reputation");

        assert_eq!(reputation.total_interactions, 12);
        assert_eq!(reputation.successful_interactions, 10);
        assert_eq!(reputation.failed_interactions, 2);
        assert!(reputation.score > 50.0, "Score should be above neutral");
    }

    #[tokio::test]
    async fn test_feedback_token_sybil_resistance() {
        let pool = setup_test_pool().await;
        let registry = KYARegistry::new(pool);

        let agent = AgentIdentity::new(
            "stellar",
            "testnet",
            "AgentWithFeedback".to_string(),
            "GAGENT123".to_string(),
        )
        .expect("Failed to create agent identity");

        let client_did = DID::new("stellar", "testnet", "client123");

        registry.register_agent(&agent).await.expect("Registration failed");

        let interaction_id = Uuid::new_v4();
        let domain = ReputationDomain::CodeAudit;

        // Issue feedback token
        let token = registry
            .issue_feedback_token(
                &agent.profile.did,
                &client_did,
                interaction_id,
                &domain,
                "test_signature".to_string(),
            )
            .await
            .expect("Failed to issue token");

        assert!(!token.used, "Token should not be used initially");

        // Submit feedback
        let result = registry
            .submit_feedback(token.id, &client_did, true, 1.0)
            .await;
        assert!(result.is_ok(), "Feedback submission failed");

        // Try to reuse token (should fail - Sybil resistance)
        let reuse_result = registry
            .submit_feedback(token.id, &client_did, true, 1.0)
            .await;
        assert!(reuse_result.is_err(), "Token reuse should be prevented");
    }

    #[tokio::test]
    async fn test_attestation_creation() {
        let pool = setup_test_pool().await;
        let registry = KYARegistry::new(pool);

        let agent = AgentIdentity::new(
            "stellar",
            "testnet",
            "AttestedAgent".to_string(),
            "GATEST123".to_string(),
        )
        .expect("Failed to create identity");

        let issuer_did = DID::new("stellar", "testnet", "issuer123");

        registry.register_agent(&agent).await.expect("Registration failed");

        let domain = ReputationDomain::CodeAudit;
        let claim = "Successfully completed 100 code audits".to_string();

        let attestation = registry
            .create_attestation(
                &agent.profile.did,
                &issuer_did,
                &domain,
                claim.clone(),
                Some("https://evidence.example.com".to_string()),
                "test_signature".to_string(),
                None,
            )
            .await
            .expect("Failed to create attestation");

        assert_eq!(attestation.claim, claim);
        assert_eq!(attestation.agent_did, agent.profile.did);

        // Retrieve attestations
        let attestations = registry
            .get_attestations(&agent.profile.did)
            .await
            .expect("Failed to get attestations");

        assert!(!attestations.is_empty(), "Should have at least one attestation");
    }

    #[tokio::test]
    async fn test_competence_proof_storage() {
        let pool = setup_test_pool().await;
        let registry = KYARegistry::new(pool);

        let agent = AgentIdentity::new(
            "stellar",
            "testnet",
            "ProofAgent".to_string(),
            "GPROOF123".to_string(),
        )
        .expect("Failed to create identity");

        registry.register_agent(&agent).await.expect("Registration failed");

        let domain = ReputationDomain::CodeAudit;
        let proof = vec![1, 2, 3, 4, 5]; // Simplified proof
        let public_inputs = vec![6, 7, 8, 9];

        let proof_record = registry
            .store_competence_proof(
                &agent.profile.did,
                &domain,
                "Test proof".to_string(),
                proof.clone(),
                public_inputs.clone(),
            )
            .await
            .expect("Failed to store proof");

        assert_eq!(proof_record.proof, proof);
        assert_eq!(proof_record.public_inputs, public_inputs);

        // Retrieve proofs
        let proofs = registry
            .get_competence_proofs(&agent.profile.did)
            .await
            .expect("Failed to get proofs");

        assert!(!proofs.is_empty(), "Should have at least one proof");
    }

    #[tokio::test]
    async fn test_modular_scoring() {
        let pool = setup_test_pool().await;
        let registry = KYARegistry::new(pool);

        let agent = AgentIdentity::new(
            "stellar",
            "testnet",
            "MultiDomainAgent".to_string(),
            "GMULTI123".to_string(),
        )
        .expect("Failed to create identity");

        registry.register_agent(&agent).await.expect("Registration failed");

        // Record interactions in multiple domains
        let domains = vec![
            ReputationDomain::CodeAudit,
            ReputationDomain::FinancialAnalysis,
            ReputationDomain::DataProcessing,
        ];

        for domain in &domains {
            registry
                .initialize_reputation(&agent.profile.did, domain)
                .await
                .expect("Failed to initialize reputation");

            for _ in 0..5 {
                registry
                    .record_interaction(&agent.profile.did, domain, true, 1.0)
                    .await
                    .expect("Failed to record interaction");
            }
        }

        // Get all scores
        let scores = registry
            .get_all_scores(&agent.profile.did)
            .await
            .expect("Failed to get scores");

        assert_eq!(scores.len(), 3, "Should have scores for 3 domains");

        // Get composite score
        let composite = registry
            .get_composite_score(&agent.profile.did)
            .await
            .expect("Failed to get composite score");

        assert!(composite > 0.0, "Composite score should be positive");
    }

    #[tokio::test]
    async fn test_cross_platform_reputation() {
        let pool = setup_test_pool().await;
        let registry = KYARegistry::new(pool);

        let agent = AgentIdentity::new(
            "stellar",
            "testnet",
            "CrossPlatformAgent".to_string(),
            "GCROSS123".to_string(),
        )
        .expect("Failed to create identity");

        registry.register_agent(&agent).await.expect("Registration failed");

        let reputation_hash = "hash123".to_string();
        let verification_proof = vec![1, 2, 3, 4];

        // Sync reputation
        let result = registry
            .sync_cross_platform_reputation(
                &agent.profile.did,
                "stellar".to_string(),
                "ethereum".to_string(),
                reputation_hash.clone(),
                verification_proof.clone(),
            )
            .await;

        assert!(result.is_ok(), "Cross-platform sync failed");

        // Retrieve cross-platform reputation
        let cross_platform = registry
            .get_cross_platform_reputation(&agent.profile.did, "stellar")
            .await
            .expect("Failed to get cross-platform reputation");

        assert!(!cross_platform.is_empty(), "Should have cross-platform data");
        assert_eq!(cross_platform[0].reputation_hash, reputation_hash);
    }

    #[tokio::test]
    async fn test_full_agent_profile() {
        let pool = setup_test_pool().await;
        let registry = KYARegistry::new(pool);

        let agent = AgentIdentity::new(
            "stellar",
            "testnet",
            "FullProfileAgent".to_string(),
            "GFULL123".to_string(),
        )
        .expect("Failed to create identity");

        registry.register_agent(&agent).await.expect("Registration failed");

        // Add reputation
        let domain = ReputationDomain::CodeAudit;
        registry
            .initialize_reputation(&agent.profile.did, &domain)
            .await
            .expect("Failed to initialize reputation");

        registry
            .record_interaction(&agent.profile.did, &domain, true, 1.0)
            .await
            .expect("Failed to record interaction");

        // Add attestation
        let issuer_did = DID::new("stellar", "testnet", "issuer");
        registry
            .create_attestation(
                &agent.profile.did,
                &issuer_did,
                &domain,
                "Test claim".to_string(),
                None,
                "signature".to_string(),
                None,
            )
            .await
            .expect("Failed to create attestation");

        // Get full profile
        let full_profile = registry
            .get_full_agent_profile(&agent.profile.did)
            .await
            .expect("Failed to get full profile");

        assert_eq!(full_profile.identity.name, "FullProfileAgent");
        assert!(!full_profile.reputations.is_empty(), "Should have reputation data");
        assert!(!full_profile.attestations.is_empty(), "Should have attestations");
        assert!(full_profile.composite_score >= 0.0, "Should have composite score");
    }
}
