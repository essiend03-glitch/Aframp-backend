use crate::kya::{error::KYAError, models::*};
use sqlx::PgPool;

/// Domain-specific reputation scoring
pub struct DomainScore {
    pool: PgPool,
}

impl DomainScore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Calculate weighted score based on multiple factors
    pub fn calculate_score(
        &self,
        total_interactions: u64,
        successful_interactions: u64,
        failed_interactions: u64,
        attestation_count: u64,
        verified_proofs: u64,
    ) -> f64 {
        if total_interactions == 0 {
            return 50.0; // Neutral score for new agents
        }

        // Success rate (0-40 points)
        let success_rate = successful_interactions as f64 / total_interactions as f64;
        let success_score = success_rate * 40.0;

        // Volume bonus (0-20 points) - logarithmic scale
        let volume_score = if total_interactions > 0 {
            (total_interactions as f64).ln().min(20.0)
        } else {
            0.0
        };

        // Attestation bonus (0-20 points)
        let attestation_score = (attestation_count as f64 * 2.0).min(20.0);

        // ZK proof bonus (0-20 points)
        let proof_score = (verified_proofs as f64 * 4.0).min(20.0);

        // Total score capped at 100
        (success_score + volume_score + attestation_score + proof_score).min(100.0)
    }

    /// Get comprehensive score with breakdown
    pub async fn get_detailed_score(
        &self,
        agent_did: &DID,
        domain: &ReputationDomain,
    ) -> Result<DetailedScore, KYAError> {
        // Get reputation data
        let rep_row = sqlx::query!(
            r#"
            SELECT total_interactions, successful_interactions, failed_interactions
            FROM kya_reputation_scores
            WHERE agent_did = $1 AND domain = $2
            "#,
            agent_did.to_string(),
            domain.as_str()
        )
        .fetch_optional(&self.pool)
        .await?;

        let (total, successful, failed) = if let Some(row) = rep_row {
            (
                row.total_interactions as u64,
                row.successful_interactions as u64,
                row.failed_interactions as u64,
            )
        } else {
            (0, 0, 0)
        };

        // Get attestation count
        let attestation_count = sqlx::query!(
            r#"
            SELECT COUNT(*) as count
            FROM kya_attestations
            WHERE agent_did = $1 AND domain = $2
            AND (expires_at IS NULL OR expires_at > NOW())
            "#,
            agent_did.to_string(),
            domain.as_str()
        )
        .fetch_one(&self.pool)
        .await?
        .count
        .unwrap_or(0) as u64;

        // Get verified proof count
        let proof_count = sqlx::query!(
            r#"
            SELECT COUNT(*) as count
            FROM kya_competence_proofs
            WHERE agent_did = $1 AND domain = $2 AND verified = true
            "#,
            agent_did.to_string(),
            domain.as_str()
        )
        .fetch_one(&self.pool)
        .await?
        .count
        .unwrap_or(0) as u64;

        let score = self.calculate_score(total, successful, failed, attestation_count, proof_count);

        Ok(DetailedScore {
            domain: domain.clone(),
            overall_score: score,
            total_interactions: total,
            successful_interactions: successful,
            failed_interactions: failed,
            success_rate: if total > 0 {
                successful as f64 / total as f64
            } else {
                0.0
            },
            attestation_count,
            verified_proof_count: proof_count,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DetailedScore {
    pub domain: ReputationDomain,
    pub overall_score: f64,
    pub total_interactions: u64,
    pub successful_interactions: u64,
    pub failed_interactions: u64,
    pub success_rate: f64,
    pub attestation_count: u64,
    pub verified_proof_count: u64,
}

/// Modular scoring system supporting multiple domains
pub struct ModularScoring {
    pool: PgPool,
}

impl ModularScoring {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get scores across all domains for an agent
    pub async fn get_all_domain_scores(&self, agent_did: &DID) -> Result<Vec<DetailedScore>, KYAError> {
        let domain_scorer = DomainScore::new(self.pool.clone());
        
        let domains = vec![
            ReputationDomain::CodeAudit,
            ReputationDomain::FinancialAnalysis,
            ReputationDomain::ContentCreation,
            ReputationDomain::DataProcessing,
            ReputationDomain::SmartContractExecution,
            ReputationDomain::PaymentProcessing,
        ];

        let mut scores = Vec::new();
        for domain in domains {
            if let Ok(score) = domain_scorer.get_detailed_score(agent_did, &domain).await {
                if score.total_interactions > 0 {
                    scores.push(score);
                }
            }
        }

        // Sort by score descending
        scores.sort_by(|a, b| b.overall_score.partial_cmp(&a.overall_score).unwrap());

        Ok(scores)
    }

    /// Calculate composite trust score across all domains
    pub async fn calculate_composite_score(&self, agent_did: &DID) -> Result<f64, KYAError> {
        let scores = self.get_all_domain_scores(agent_did).await?;
        
        if scores.is_empty() {
            return Ok(50.0); // Neutral score for new agents
        }

        // Weighted average based on interaction volume
        let total_weight: u64 = scores.iter().map(|s| s.total_interactions).sum();
        if total_weight == 0 {
            return Ok(50.0);
        }

        let weighted_sum: f64 = scores
            .iter()
            .map(|s| s.overall_score * s.total_interactions as f64)
            .sum();

        Ok(weighted_sum / total_weight as f64)
    }

    /// Get agent ranking in a specific domain
    pub async fn get_domain_ranking(
        &self,
        agent_did: &DID,
        domain: &ReputationDomain,
    ) -> Result<DomainRanking, KYAError> {
        let agent_score = sqlx::query!(
            r#"
            SELECT score
            FROM kya_reputation_scores
            WHERE agent_did = $1 AND domain = $2
            "#,
            agent_did.to_string(),
            domain.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|r| r.score)
        .unwrap_or(50.0);

        let total_agents = sqlx::query!(
            r#"
            SELECT COUNT(DISTINCT agent_did) as count
            FROM kya_reputation_scores
            WHERE domain = $1
            "#,
            domain.as_str()
        )
        .fetch_one(&self.pool)
        .await?
        .count
        .unwrap_or(0) as u64;

        let rank = sqlx::query!(
            r#"
            SELECT COUNT(*) as rank
            FROM kya_reputation_scores
            WHERE domain = $1 AND score > $2
            "#,
            domain.as_str(),
            agent_score
        )
        .fetch_one(&self.pool)
        .await?
        .rank
        .unwrap_or(0) as u64 + 1;

        let percentile = if total_agents > 0 {
            ((total_agents - rank) as f64 / total_agents as f64) * 100.0
        } else {
            50.0
        };

        Ok(DomainRanking {
            domain: domain.clone(),
            score: agent_score,
            rank,
            total_agents,
            percentile,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DomainRanking {
    pub domain: ReputationDomain,
    pub score: f64,
    pub rank: u64,
    pub total_agents: u64,
    pub percentile: f64,
}
