use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::AppError;
use super::{ProtocolConfig, RiskTier, EvaluationScores, ProtocolHealthMetrics};

/// DeFi protocol evaluation framework
/// 
/// Provides a systematic approach to evaluating and classifying DeFi protocols
/// based on comprehensive criteria including TVL, protocol age, audit history,
/// team reputation, and regulatory compliance.
pub struct ProtocolEvaluator {
    evaluation_criteria: EvaluationCriteria,
}

impl ProtocolEvaluator {
    pub fn new(evaluation_criteria: EvaluationCriteria) -> Self {
        Self {
            evaluation_criteria,
        }
    }

    /// Evaluate a protocol and assign risk tier
    pub async fn evaluate_protocol(
        &self,
        protocol_config: &ProtocolConfig,
        health_metrics: &ProtocolHealthMetrics,
    ) -> Result<EvaluationResult, AppError> {
        let scores = self.calculate_evaluation_scores(protocol_config, health_metrics).await?;
        let risk_tier = self.determine_risk_tier(&scores);
        let recommendation = self.generate_recommendation(&scores, &risk_tier);

        Ok(EvaluationResult {
            protocol_id: protocol_config.protocol_id.clone(),
            evaluation_date: Utc::now(),
            scores,
            risk_tier,
            recommendation,
            evaluation_summary: self.generate_evaluation_summary(&scores, &risk_tier),
        })
    }

    /// Calculate evaluation scores for a protocol
    async fn calculate_evaluation_scores(
        &self,
        protocol_config: &ProtocolConfig,
        health_metrics: &ProtocolHealthMetrics,
    ) -> Result<EvaluationScores, AppError> {
        let tvl_score = self.evaluate_tvl(health_metrics.total_value_locked.clone());
        let age_score = self.evaluate_protocol_age(protocol_config.created_at);
        let audit_score = self.evaluate_audit_history(protocol_config).await?;
        let team_score = self.evaluate_team_reputation(protocol_config).await?;
        let codebase_score = self.evaluate_codebase_quality(protocol_config).await?;
        let governance_score = self.evaluate_governance_model(protocol_config).await?;
        let compliance_score = self.evaluate_regulatory_compliance(protocol_config).await?;
        let ecosystem_score = self.evaluate_ecosystem_integration(protocol_config).await?;

        // Calculate weighted total score
        let total_score = 
            tvl_score * self.evaluation_criteria.tvl_weight +
            age_score * self.evaluation_criteria.age_weight +
            audit_score * self.evaluation_criteria.audit_weight +
            team_score * self.evaluation_criteria.team_weight +
            codebase_score * self.evaluation_criteria.codebase_weight +
            governance_score * self.evaluation_criteria.governance_weight +
            compliance_score * self.evaluation_criteria.compliance_weight +
            ecosystem_score * self.evaluation_criteria.ecosystem_weight;

        Ok(EvaluationScores {
            tvl_score,
            age_score,
            audit_score,
            team_score,
            codebase_score,
            governance_score,
            compliance_score,
            ecosystem_score,
            total_score,
        })
    }

    /// Evaluate Total Value Locked (TVL)
    fn evaluate_tvl(&self, tvl: BigDecimal) -> f64 {
        let tvl_f64: f64 = tvl.to_string().parse().unwrap_or(0.0);
        
        if tvl_f64 >= self.evaluation_criteria.min_tvl_tier1 {
            1.0
        } else if tvl_f64 >= self.evaluation_criteria.min_tvl_tier2 {
            0.7
        } else {
            0.3
        }
    }

    /// Evaluate protocol age
    fn evaluate_protocol_age(&self, created_at: DateTime<Utc>) -> f64 {
        let age_days = (Utc::now() - created_at).num_days();
        
        if age_days >= self.evaluation_criteria.min_age_days_tier1 {
            1.0
        } else if age_days >= self.evaluation_criteria.min_age_days_tier2 {
            0.7
        } else {
            0.3
        }
    }

    /// Evaluate audit history (placeholder implementation)
    async fn evaluate_audit_history(&self, _protocol_config: &ProtocolConfig) -> Result<f64, AppError> {
        // In a real implementation, this would check audit reports, number of audits,
        // audit firms reputation, and audit findings
        Ok(0.8) // Default score for demonstration
    }

    /// Evaluate team reputation (placeholder implementation)
    async fn evaluate_team_reputation(&self, _protocol_config: &ProtocolConfig) -> Result<f64, AppError> {
        // In a real implementation, this would check team background, experience,
        // and track record in DeFi
        Ok(0.7) // Default score for demonstration
    }

    /// Evaluate codebase quality (placeholder implementation)
    async fn evaluate_codebase_quality(&self, _protocol_config: &ProtocolConfig) -> Result<f64, AppError> {
        // In a real implementation, this would check if code is open source,
        // has been reviewed, and follows best practices
        Ok(0.8) // Default score for demonstration
    }

    /// Evaluate governance model (placeholder implementation)
    async fn evaluate_governance_model(&self, _protocol_config: &ProtocolConfig) -> Result<f64, AppError> {
        // In a real implementation, this would evaluate governance mechanisms,
        // decentralization level, and community involvement
        Ok(0.6) // Default score for demonstration
    }

    /// Evaluate regulatory compliance (placeholder implementation)
    async fn evaluate_regulatory_compliance(&self, _protocol_config: &ProtocolConfig) -> Result<f64, AppError> {
        // In a real implementation, this would check regulatory compliance,
        // licenses, and legal frameworks
        Ok(0.7) // Default score for demonstration
    }

    /// Evaluate ecosystem integration (placeholder implementation)
    async fn evaluate_ecosystem_integration(&self, _protocol_config: &ProtocolConfig) -> Result<f64, AppError> {
        // In a real implementation, this would check integration with other DeFi
        // protocols, wallet support, and ecosystem adoption
        Ok(0.8) // Default score for demonstration
    }

    /// Determine risk tier based on evaluation scores
    fn determine_risk_tier(&self, scores: &EvaluationScores) -> RiskTier {
        if scores.total_score >= self.evaluation_criteria.tier1_min_score {
            RiskTier::Tier1
        } else if scores.total_score >= self.evaluation_criteria.tier2_min_score {
            RiskTier::Tier2
        } else {
            RiskTier::Tier3
        }
    }

    /// Generate recommendation based on evaluation
    fn generate_recommendation(&self, scores: &EvaluationScores, risk_tier: &RiskTier) -> String {
        match risk_tier {
            RiskTier::Tier1 => {
                format!(
                    "Protocol approved for full integration. Excellent scores across all criteria (Total: {:.2}).",
                    scores.total_score
                )
            }
            RiskTier::Tier2 => {
                format!(
                    "Protocol approved for limited integration. Moderate scores (Total: {:.2}). Monitor closely.",
                    scores.total_score
                )
            }
            RiskTier::Tier3 => {
                format!(
                    "Protocol rejected for integration. Insufficient scores (Total: {:.2}). Consider for future evaluation.",
                    scores.total_score
                )
            }
        }
    }

    /// Generate evaluation summary
    fn generate_evaluation_summary(&self, scores: &EvaluationScores, risk_tier: &RiskTier) -> String {
        format!(
            "Protocol evaluated and classified as {:?}. Key scores: TVL: {:.2}, Age: {:.2}, Audit: {:.2}, Team: {:.2}, Total: {:.2}",
            risk_tier, scores.tvl_score, scores.age_score, scores.audit_score, scores.team_score, scores.total_score
        )
    }
}

/// Evaluation criteria configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationCriteria {
    // TVL thresholds (in USD)
    pub min_tvl_tier1: f64,
    pub min_tvl_tier2: f64,
    
    // Age thresholds (in days)
    pub min_age_days_tier1: i64,
    pub min_age_days_tier2: i64,
    
    // Score thresholds
    pub tier1_min_score: f64,
    pub tier2_min_score: f64,
    
    // Evaluation weights (must sum to 1.0)
    pub tvl_weight: f64,
    pub age_weight: f64,
    pub audit_weight: f64,
    pub team_weight: f64,
    pub codebase_weight: f64,
    pub governance_weight: f64,
    pub compliance_weight: f64,
    pub ecosystem_weight: f64,
}

impl Default for EvaluationCriteria {
    fn default() -> Self {
        Self {
            min_tvl_tier1: 100_000_000.0, // $100M
            min_tvl_tier2: 10_000_000.0,   // $10M
            min_age_days_tier1: 365,       // 1 year
            min_age_days_tier2: 90,        // 3 months
            tier1_min_score: 0.8,
            tier2_min_score: 0.6,
            tvl_weight: 0.15,
            age_weight: 0.10,
            audit_weight: 0.20,
            team_weight: 0.15,
            codebase_weight: 0.10,
            governance_weight: 0.10,
            compliance_weight: 0.10,
            ecosystem_weight: 0.10,
        }
    }
}

/// Result of protocol evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub protocol_id: String,
    pub evaluation_date: DateTime<Utc>,
    pub scores: EvaluationScores,
    pub risk_tier: RiskTier,
    pub recommendation: String,
    pub evaluation_summary: String,
}

/// Protocol evaluation history
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProtocolEvaluationHistory {
    pub evaluation_id: Uuid,
    pub protocol_id: String,
    pub evaluation_date: DateTime<Utc>,
    pub scores: EvaluationScores,
    pub risk_tier: RiskTier,
    pub recommendation: String,
    pub evaluator_id: String,
    pub created_at: DateTime<Utc>,
}

/// Protocol evaluator for automated evaluations
pub struct AutomatedEvaluator {
    evaluator: ProtocolEvaluator,
}

impl AutomatedEvaluator {
    pub fn new(criteria: EvaluationCriteria) -> Self {
        Self {
            evaluator: ProtocolEvaluator::new(criteria),
        }
    }

    /// Batch evaluate all active protocols
    pub async fn evaluate_all_protocols(
        &self,
        protocols: &[ProtocolConfig],
        health_metrics: &HashMap<String, ProtocolHealthMetrics>,
    ) -> Result<Vec<EvaluationResult>, AppError> {
        let mut results = Vec::new();

        for protocol in protocols {
            if let Some(metrics) = health_metrics.get(&protocol.protocol_id) {
                let result = self.evaluator.evaluate_protocol(protocol, metrics).await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Check if any protocol needs re-evaluation based on time or significant changes
    pub async fn check_revaluation_needed(
        &self,
        protocol: &ProtocolConfig,
        health_metrics: &ProtocolHealthMetrics,
        last_evaluation: Option<&ProtocolEvaluationHistory>,
    ) -> bool {
        // Re-evaluate if no previous evaluation
        if last_evaluation.is_none() {
            return true;
        }

        let last_eval = last_evaluation.unwrap();
        
        // Re-evaluate if more than 30 days have passed
        let days_since_eval = (Utc::now() - last_eval.evaluation_date).num_days();
        if days_since_eval > 30 {
            return true;
        }

        // Re-evaluate if TVL changed significantly (>20%)
        let tvl_change_pct = health_metrics.tvl_change_24h.abs();
        if tvl_change_pct > 0.2 {
            return true;
        }

        // Re-evaluate if health score dropped significantly
        if health_metrics.health_score < 0.5 {
            return true;
        }

        false
    }
}
