use crate::kya::{
    attestation::{Attestation, AttestationVerifier},
    error::KYAError,
    identity::IdentityRegistry,
    models::*,
    reputation::{FeedbackAuthorization, ReputationManager},
    scoring::{DomainScore, ModularScoring},
    zkp::{CompetenceProof, ZKProofVerifier},
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/// Central registry coordinating all KYA components
pub struct KYARegistry {
    pool: PgPool,
    identity_registry: IdentityRegistry,
    reputation_manager: ReputationManager,
    feedback_auth: FeedbackAuthorization,
    attestation: Attestation,
    attestation_verifier: AttestationVerifier,
    competence_proof: CompetenceProof,
    zkp_verifier: ZKProofVerifier,
    domain_scorer: DomainScore,
    modular_scoring: ModularScoring,
}

impl KYARegistry {
    pub fn new(pool: PgPool) -> Self {
        Self {
            identity_registry: IdentityRegistry::new(pool.clone()),
            reputation_manager: ReputationManager::new(pool.clone()),
            feedback_auth: FeedbackAuthorization::new(pool.clone()),
            attestation: Attestation::new(pool.clone()),
            attestation_verifier: AttestationVerifier::new(pool.clone()),
            competence_proof: CompetenceProof::new(pool.clone()),
            zkp_verifier: ZKProofVerifier::new(pool.clone()),
            domain_scorer: DomainScore::new(pool.clone()),
            modular_scoring: ModularScoring::new(pool.clone()),
            pool,
        }
    }

    // Identity Management
    pub async fn register_agent(&self, identity: &crate::kya::identity::AgentIdentity) -> Result<(), KYAError> {
        self.identity_registry.register(identity).await
    }

    pub async fn get_agent(&self, did: &DID) -> Result<crate::kya::identity::AgentIdentity, KYAError> {
        self.identity_registry.get_by_did(did).await
    }

    pub async fn update_agent_profile(&self, profile: &AgentProfile) -> Result<(), KYAError> {
        self.identity_registry.update_profile(profile).await
    }

    pub async fn list_agents(&self, limit: i64, offset: i64) -> Result<Vec<crate::kya::identity::AgentIdentity>, KYAError> {
        self.identity_registry.list_agents(limit, offset).await
    }

    // Reputation Management
    pub async fn initialize_reputation(&self, agent_did: &DID, domain: &ReputationDomain) -> Result<(), KYAError> {
        self.reputation_manager.initialize_domain_reputation(agent_did, domain).await
    }

    pub async fn get_reputation(&self, agent_did: &DID, domain: &ReputationDomain) -> Result<DomainReputationScore, KYAError> {
        self.reputation_manager.get_domain_score(agent_did, domain).await
    }

    pub async fn get_all_reputations(&self, agent_did: &DID) -> Result<Vec<DomainReputationScore>, KYAError> {
        self.reputation_manager.get_all_scores(agent_did).await
    }

    pub async fn record_interaction(
        &self,
        agent_did: &DID,
        domain: &ReputationDomain,
        success: bool,
        weight: f64,
    ) -> Result<(), KYAError> {
        self.reputation_manager.record_interaction(agent_did, domain, success, weight).await
    }

    // Feedback Authorization (Sybil Resistance)
    pub async fn issue_feedback_token(
        &self,
        agent_did: &DID,
        client_did: &DID,
        interaction_id: Uuid,
        domain: &ReputationDomain,
        signature: String,
    ) -> Result<FeedbackToken, KYAError> {
        self.feedback_auth.issue_token(agent_did, client_did, interaction_id, domain, signature).await
    }

    pub async fn submit_feedback(
        &self,
        token_id: Uuid,
        client_did: &DID,
        success: bool,
        weight: f64,
    ) -> Result<(), KYAError> {
        let token = self.feedback_auth.verify_and_consume(token_id, client_did).await?;
        self.reputation_manager.record_interaction(&token.agent_did, &token.domain, success, weight).await
    }

    // Attestations
    pub async fn create_attestation(
        &self,
        agent_did: &DID,
        issuer_did: &DID,
        domain: &ReputationDomain,
        claim: String,
        evidence_uri: Option<String>,
        signature: String,
        expires_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<AttestationRecord, KYAError> {
        self.attestation.create(agent_did, issuer_did, domain, claim, evidence_uri, signature, expires_at).await
    }

    pub async fn get_attestations(&self, agent_did: &DID) -> Result<Vec<AttestationRecord>, KYAError> {
        self.attestation.get_by_agent(agent_did).await
    }

    pub async fn verify_attestation(&self, attestation: &AttestationRecord) -> Result<bool, KYAError> {
        self.attestation_verifier.verify(attestation).await
    }

    // Zero-Knowledge Proofs
    pub async fn store_competence_proof(
        &self,
        agent_did: &DID,
        domain: &ReputationDomain,
        claim: String,
        proof: Vec<u8>,
        public_inputs: Vec<u8>,
    ) -> Result<CompetenceProofRecord, KYAError> {
        self.competence_proof.store_proof(agent_did, domain, claim, proof, public_inputs).await
    }

    pub async fn get_competence_proofs(&self, agent_did: &DID) -> Result<Vec<CompetenceProofRecord>, KYAError> {
        self.competence_proof.get_by_agent(agent_did).await
    }

    pub async fn verify_competence_proof(
        &self,
        proof_record: &CompetenceProofRecord,
        expected_public_inputs: &[u8],
    ) -> Result<bool, KYAError> {
        self.zkp_verifier.verify_proof(proof_record, expected_public_inputs)
    }

    // Scoring
    pub async fn get_detailed_score(&self, agent_did: &DID, domain: &ReputationDomain) -> Result<crate::kya::scoring::DetailedScore, KYAError> {
        self.domain_scorer.get_detailed_score(agent_did, domain).await
    }

    pub async fn get_all_scores(&self, agent_did: &DID) -> Result<Vec<crate::kya::scoring::DetailedScore>, KYAError> {
        self.modular_scoring.get_all_domain_scores(agent_did).await
    }

    pub async fn get_composite_score(&self, agent_did: &DID) -> Result<f64, KYAError> {
        self.modular_scoring.calculate_composite_score(agent_did).await
    }

    pub async fn get_ranking(&self, agent_did: &DID, domain: &ReputationDomain) -> Result<crate::kya::scoring::DomainRanking, KYAError> {
        self.modular_scoring.get_domain_ranking(agent_did, domain).await
    }

    // Cross-Platform Reputation
    pub async fn sync_cross_platform_reputation(
        &self,
        agent_did: &DID,
        source_platform: String,
        target_platform: String,
        reputation_hash: String,
        verification_proof: Vec<u8>,
    ) -> Result<(), KYAError> {
        sqlx::query!(
            r#"
            INSERT INTO kya_cross_platform_reputation
            (agent_did, source_platform, target_platform, reputation_hash, verification_proof, synced_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (agent_did, source_platform, target_platform)
            DO UPDATE SET reputation_hash = $4, verification_proof = $5, synced_at = $6
            "#,
            agent_did.to_string(),
            source_platform,
            target_platform,
            reputation_hash,
            verification_proof,
            Utc::now()
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_cross_platform_reputation(
        &self,
        agent_did: &DID,
        source_platform: &str,
    ) -> Result<Vec<CrossPlatformReputation>, KYAError> {
        let rows = sqlx::query!(
            r#"
            SELECT agent_did, source_platform, target_platform, reputation_hash, 
                   verification_proof, synced_at
            FROM kya_cross_platform_reputation
            WHERE agent_did = $1 AND source_platform = $2
            "#,
            agent_did.to_string(),
            source_platform
        )
        .fetch_all(&self.pool)
        .await?;

        let mut reputations = Vec::new();
        for row in rows {
            reputations.push(CrossPlatformReputation {
                agent_did: DID::from_string(&row.agent_did)?,
                source_platform: row.source_platform,
                target_platform: row.target_platform,
                reputation_hash: row.reputation_hash,
                verification_proof: row.verification_proof,
                synced_at: row.synced_at,
            });
        }

        Ok(reputations)
    }

    /// Get comprehensive agent profile with all reputation data
    pub async fn get_full_agent_profile(&self, agent_did: &DID) -> Result<FullAgentProfile, KYAError> {
        let identity = self.get_agent(agent_did).await?;
        let reputations = self.get_all_reputations(agent_did).await?;
        let attestations = self.get_attestations(agent_did).await?;
        let proofs = self.get_competence_proofs(agent_did).await?;
        let composite_score = self.get_composite_score(agent_did).await?;

        Ok(FullAgentProfile {
            identity: identity.export_profile(),
            reputations,
            attestations,
            competence_proofs: proofs,
            composite_score,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FullAgentProfile {
    pub identity: AgentProfile,
    pub reputations: Vec<DomainReputationScore>,
    pub attestations: Vec<AttestationRecord>,
    pub competence_proofs: Vec<CompetenceProofRecord>,
    pub composite_score: f64,
}
