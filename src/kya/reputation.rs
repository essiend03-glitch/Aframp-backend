use crate::kya::{error::KYAError, models::*};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

/// Manages agent reputation scores and feedback
pub struct ReputationManager {
    pool: PgPool,
}

impl ReputationManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize reputation for a new agent in a domain
    pub async fn initialize_domain_reputation(
        &self,
        agent_did: &DID,
        domain: &ReputationDomain,
    ) -> Result<(), KYAError> {
        sqlx::query!(
            r#"
            INSERT INTO kya_reputation_scores 
            (agent_did, domain, score, total_interactions, successful_interactions, 
             failed_interactions, last_updated)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (agent_did, domain) DO NOTHING
            "#,
            agent_did.to_string(),
            domain.as_str(),
            50.0, // Start with neutral score
            0i64,
            0i64,
            0i64,
            Utc::now()
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get reputation score for an agent in a specific domain
    pub async fn get_domain_score(
        &self,
        agent_did: &DID,
        domain: &ReputationDomain,
    ) -> Result<DomainReputationScore, KYAError> {
        let row = sqlx::query!(
            r#"
            SELECT domain, score, total_interactions, successful_interactions,
                   failed_interactions, last_updated
            FROM kya_reputation_scores
            WHERE agent_did = $1 AND domain = $2
            "#,
            agent_did.to_string(),
            domain.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| KYAError::InvalidReputationScore)?;

        Ok(DomainReputationScore {
            domain: domain.clone(),
            score: row.score,
            total_interactions: row.total_interactions as u64,
            successful_interactions: row.successful_interactions as u64,
            failed_interactions: row.failed_interactions as u64,
            last_updated: row.last_updated,
        })
    }

    /// Get all reputation scores for an agent
    pub async fn get_all_scores(&self, agent_did: &DID) -> Result<Vec<DomainReputationScore>, KYAError> {
        let rows = sqlx::query!(
            r#"
            SELECT domain, score, total_interactions, successful_interactions,
                   failed_interactions, last_updated
            FROM kya_reputation_scores
            WHERE agent_did = $1
            ORDER BY score DESC
            "#,
            agent_did.to_string()
        )
        .fetch_all(&self.pool)
        .await?;

        let mut scores = Vec::new();
        for row in rows {
            let domain = match row.domain.as_str() {
                "code_audit" => ReputationDomain::CodeAudit,
                "financial_analysis" => ReputationDomain::FinancialAnalysis,
                "content_creation" => ReputationDomain::ContentCreation,
                "data_processing" => ReputationDomain::DataProcessing,
                "smart_contract_execution" => ReputationDomain::SmartContractExecution,
                "payment_processing" => ReputationDomain::PaymentProcessing,
                custom => ReputationDomain::Custom(custom.to_string()),
            };

            scores.push(DomainReputationScore {
                domain,
                score: row.score,
                total_interactions: row.total_interactions as u64,
                successful_interactions: row.successful_interactions as u64,
                failed_interactions: row.failed_interactions as u64,
                last_updated: row.last_updated,
            });
        }

        Ok(scores)
    }

    /// Update reputation based on interaction outcome
    pub async fn record_interaction(
        &self,
        agent_did: &DID,
        domain: &ReputationDomain,
        success: bool,
        weight: f64,
    ) -> Result<(), KYAError> {
        // Ensure domain exists
        self.initialize_domain_reputation(agent_did, domain).await?;

        let score_delta = if success { weight } else { -weight };

        sqlx::query!(
            r#"
            UPDATE kya_reputation_scores
            SET 
                total_interactions = total_interactions + 1,
                successful_interactions = successful_interactions + $1,
                failed_interactions = failed_interactions + $2,
                score = GREATEST(0.0, LEAST(100.0, score + $3)),
                last_updated = $4
            WHERE agent_did = $5 AND domain = $6
            "#,
            if success { 1i64 } else { 0i64 },
            if success { 0i64 } else { 1i64 },
            score_delta,
            Utc::now(),
            agent_did.to_string(),
            domain.as_str()
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Manages feedback authorization tokens (Sybil resistance)
pub struct FeedbackAuthorization {
    pool: PgPool,
}

impl FeedbackAuthorization {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Issue a feedback token after a verified interaction
    pub async fn issue_token(
        &self,
        agent_did: &DID,
        client_did: &DID,
        interaction_id: Uuid,
        domain: &ReputationDomain,
        signature: String,
    ) -> Result<FeedbackToken, KYAError> {
        let token = FeedbackToken {
            id: Uuid::new_v4(),
            agent_did: agent_did.clone(),
            client_did: client_did.clone(),
            interaction_id,
            domain: domain.clone(),
            authorized_at: Utc::now(),
            used: false,
            signature,
        };

        sqlx::query!(
            r#"
            INSERT INTO kya_feedback_tokens
            (id, agent_did, client_did, interaction_id, domain, authorized_at, used, signature)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            token.id,
            token.agent_did.to_string(),
            token.client_did.to_string(),
            token.interaction_id,
            token.domain.as_str(),
            token.authorized_at,
            token.used,
            token.signature
        )
        .execute(&self.pool)
        .await?;

        Ok(token)
    }

    /// Verify and consume a feedback token
    pub async fn verify_and_consume(
        &self,
        token_id: Uuid,
        client_did: &DID,
    ) -> Result<FeedbackToken, KYAError> {
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query!(
            r#"
            SELECT id, agent_did, client_did, interaction_id, domain, 
                   authorized_at, used, signature
            FROM kya_feedback_tokens
            WHERE id = $1 AND client_did = $2
            FOR UPDATE
            "#,
            token_id,
            client_did.to_string()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(KYAError::UnauthorizedFeedback)?;

        if row.used {
            return Err(KYAError::SybilAttackDetected);
        }

        sqlx::query!(
            r#"
            UPDATE kya_feedback_tokens
            SET used = true
            WHERE id = $1
            "#,
            token_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let domain = match row.domain.as_str() {
            "code_audit" => ReputationDomain::CodeAudit,
            "financial_analysis" => ReputationDomain::FinancialAnalysis,
            "content_creation" => ReputationDomain::ContentCreation,
            "data_processing" => ReputationDomain::DataProcessing,
            "smart_contract_execution" => ReputationDomain::SmartContractExecution,
            "payment_processing" => ReputationDomain::PaymentProcessing,
            custom => ReputationDomain::Custom(custom.to_string()),
        };

        Ok(FeedbackToken {
            id: row.id,
            agent_did: DID::from_string(&row.agent_did)?,
            client_did: DID::from_string(&row.client_did)?,
            interaction_id: row.interaction_id,
            domain,
            authorized_at: row.authorized_at,
            used: true,
            signature: row.signature,
        })
    }

    /// Check if a client has already submitted feedback for an interaction
    pub async fn has_submitted_feedback(
        &self,
        interaction_id: Uuid,
        client_did: &DID,
    ) -> Result<bool, KYAError> {
        let count = sqlx::query!(
            r#"
            SELECT COUNT(*) as count
            FROM kya_feedback_tokens
            WHERE interaction_id = $1 AND client_did = $2 AND used = true
            "#,
            interaction_id,
            client_did.to_string()
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.count.unwrap_or(0) > 0)
    }
}
