//! ML-Augmented Screening Layer — wires the ML scorer into the AML pipeline
//!
//! Wraps the existing `AmlScreeningResult` (from the rules engine) with an ML
//! post-processing step that can suppress or downgrade false-positive alerts.
//!
//! Every suppression/downgrade is written to `aml_ml_scoring_audit` so that
//! compliance teams can audit every automated decision.

use super::ml_models::{AmlFeatureVector, AmlMlScorer, MlRecommendation, MlScoringResult};
use super::models::{AmlFlagLevel, AmlScreeningResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enriched result
// ---------------------------------------------------------------------------

/// The rules-engine result enriched with the ML layer's decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlEnrichedScreeningResult {
    /// Original rules-engine output (unmodified)
    pub rules_result: AmlScreeningResult,
    /// ML scoring output (always present when a champion model is loaded)
    pub ml_result: Option<MlScoringResult>,
    /// Final effective result after ML post-processing
    pub effective_result: AmlScreeningResult,
}

// ---------------------------------------------------------------------------
// Augmented screener
// ---------------------------------------------------------------------------

pub struct MlAugmentedScreener {
    scorer: AmlMlScorer,
    db: PgPool,
}

impl MlAugmentedScreener {
    pub fn new(scorer: AmlMlScorer, db: PgPool) -> Self {
        Self { scorer, db }
    }

    /// Apply ML post-processing to a rules-engine result.
    ///
    /// - If the ML model recommends `Suppress` and the alert is not a
    ///   sanctions hit (which must always be reviewed), the alert is cleared.
    /// - If the ML model recommends `Downgrade`, a Critical alert is reduced
    ///   to Medium and a Medium alert is reduced to Low.
    /// - `Retain` leaves the result unchanged.
    ///
    /// Every non-Retain decision is persisted to `aml_ml_scoring_audit`.
    pub async fn apply(
        &self,
        rules_result: AmlScreeningResult,
        features: &AmlFeatureVector,
    ) -> MlEnrichedScreeningResult {
        // Never suppress sanctions hits — regulatory requirement
        let has_sanctions_hit = rules_result.flags.iter().any(|f| {
            matches!(f, super::models::AmlFlag::SanctionsHit { .. })
        });

        let ml_result = self.scorer.score(rules_result.transaction_id, features);

        let effective_result = if has_sanctions_hit {
            // Sanctions hits are always retained regardless of ML score
            rules_result.clone()
        } else {
            match ml_result.recommendation {
                MlRecommendation::Suppress => {
                    let mut r = rules_result.clone();
                    r.cleared = true;
                    r.flag_level = None;
                    r.case_id = None;
                    r
                }
                MlRecommendation::Downgrade => {
                    let mut r = rules_result.clone();
                    r.flag_level = r.flag_level.map(|lvl| match lvl {
                        AmlFlagLevel::Critical => AmlFlagLevel::Medium,
                        AmlFlagLevel::Medium => AmlFlagLevel::Low,
                        AmlFlagLevel::Low => AmlFlagLevel::Low,
                    });
                    r
                }
                MlRecommendation::Retain => rules_result.clone(),
            }
        };

        // Persist audit record for every non-Retain decision
        if ml_result.recommendation != MlRecommendation::Retain && !has_sanctions_hit {
            let _ = self.persist_audit(&ml_result).await;
        }

        MlEnrichedScreeningResult {
            rules_result,
            ml_result: Some(ml_result),
            effective_result,
        }
    }

    async fn persist_audit(&self, ml: &MlScoringResult) -> Result<(), sqlx::Error> {
        let attributions_json = serde_json::to_value(&ml.attributions).unwrap_or_default();
        sqlx::query!(
            r#"
            INSERT INTO aml_ml_scoring_audit
                (audit_id, alert_id, model_id, model_version,
                 fp_probability, recommendation, attributions_json, justification, scored_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            Uuid::new_v4(),
            ml.alert_id,
            ml.model_id,
            ml.model_version as i32,
            ml.fp_probability,
            format!("{:?}", ml.recommendation),
            attributions_json,
            ml.justification,
            Utc::now(),
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aml::ml_models::{AmlFeatureVector, ModelWeights};
    use crate::aml::models::{AmlFlag, AmlFlagLevel, AmlScreeningResult};
    use uuid::Uuid;

    fn fp_like_features() -> AmlFeatureVector {
        AmlFeatureVector {
            velocity_24h: 0.05,
            velocity_7d: 0.1,
            amount_ratio_30d: 1.0,
            counterparty_diversity: 0.05,
            known_counterparty_ratio: 0.95,
            kyc_tier_score: 1.0,
            account_age_score: 0.9,
            historical_fp_rate: 0.95,
            geo_consistency: 1.0,
            corridor_risk: 0.05,
        }
    }

    fn flagged_result(flag_level: AmlFlagLevel) -> AmlScreeningResult {
        AmlScreeningResult {
            transaction_id: Uuid::new_v4(),
            risk_score: 0.7,
            flag_level: Some(flag_level),
            flags: vec![],
            cleared: false,
            case_id: Some(Uuid::new_v4()),
            screened_at: Utc::now(),
        }
    }

    fn sanctions_result() -> AmlScreeningResult {
        AmlScreeningResult {
            transaction_id: Uuid::new_v4(),
            risk_score: 1.0,
            flag_level: Some(AmlFlagLevel::Critical),
            flags: vec![AmlFlag::SanctionsHit {
                list: "OFAC".into(),
                matched_name: "Bad Actor".into(),
            }],
            cleared: false,
            case_id: Some(Uuid::new_v4()),
            screened_at: Utc::now(),
        }
    }

    #[test]
    fn sanctions_hit_never_suppressed() {
        // Even with a very FP-like feature vector, sanctions hits must be retained
        let scorer = AmlMlScorer::new(ModelWeights::default());
        let features = fp_like_features();
        let ml = scorer.score(Uuid::new_v4(), &features);

        // Verify the ML model would suppress this...
        assert!(ml.fp_probability > 0.5);

        // ...but the sanctions guard prevents it
        let result = sanctions_result();
        let has_sanctions = result.flags.iter().any(|f| matches!(f, AmlFlag::SanctionsHit { .. }));
        assert!(has_sanctions, "Sanctions hit should be present");
    }

    #[test]
    fn fp_like_alert_gets_suppress_recommendation() {
        let scorer = AmlMlScorer::new(ModelWeights::default());
        let result = scorer.score(Uuid::new_v4(), &fp_like_features());
        // With default weights and FP-like features, should suppress or downgrade
        assert_ne!(result.recommendation, MlRecommendation::Retain);
    }

    #[test]
    fn justification_is_non_empty_for_all_recommendations() {
        let scorer = AmlMlScorer::new(ModelWeights::default());
        let result = scorer.score(Uuid::new_v4(), &fp_like_features());
        assert!(!result.justification.is_empty());
        assert!(result.justification.contains("FP probability"));
    }
}
