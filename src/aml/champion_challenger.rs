//! Champion/Challenger Framework — Safe Model Promotion
//!
//! Implements the A/B testing framework required by the issue spec:
//!
//! - **Shadow mode**: challenger model scores every live alert but its
//!   recommendation is NOT acted upon.  Results are logged to
//!   `aml_shadow_evaluations` for offline comparison.
//! - **A/B routing**: a configurable percentage of traffic is routed to the
//!   challenger so its recommendations ARE acted upon (canary deployment).
//! - **Promotion**: when the challenger's FP rate is ≥30% better than the
//!   champion's over a sufficient sample, it can be promoted to champion.

use super::ml_models::{AmlFeatureVector, AmlMlScorer, ModelWeights, MlScoringResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChampionChallengerConfig {
    /// Fraction of live traffic routed to challenger (0.0 = shadow only, 1.0 = full rollout)
    pub challenger_traffic_fraction: f64,
    /// Minimum shadow evaluations before promotion is allowed
    pub min_shadow_evaluations: u64,
    /// Required FP-rate improvement (relative) to auto-promote
    pub required_fp_improvement: f64,
}

impl Default for ChampionChallengerConfig {
    fn default() -> Self {
        Self {
            challenger_traffic_fraction: 0.0, // shadow-only by default
            min_shadow_evaluations: 500,
            required_fp_improvement: 0.30, // 30% reduction required
        }
    }
}

// ---------------------------------------------------------------------------
// Shadow evaluation record
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowEvaluation {
    pub eval_id: Uuid,
    pub alert_id: Uuid,
    pub champion_model_id: Uuid,
    pub challenger_model_id: Uuid,
    pub champion_fp_probability: f64,
    pub challenger_fp_probability: f64,
    pub champion_recommendation: String,
    pub challenger_recommendation: String,
    /// Whether the alert was later confirmed as a false positive by an analyst
    pub analyst_confirmed_fp: Option<bool>,
    pub evaluated_at: chrono::DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Promotion result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PromotionDecision {
    Promoted {
        new_champion_id: Uuid,
        old_champion_id: Uuid,
        challenger_fp_rate: f64,
        champion_fp_rate: f64,
        improvement_pct: f64,
    },
    Rejected {
        reason: String,
        challenger_fp_rate: f64,
        champion_fp_rate: f64,
    },
    InsufficientData {
        shadow_evaluations: u64,
        required: u64,
    },
}

// ---------------------------------------------------------------------------
// Framework
// ---------------------------------------------------------------------------

pub struct ChampionChallengerFramework {
    db: PgPool,
    config: ChampionChallengerConfig,
    champion: Arc<RwLock<AmlMlScorer>>,
    champion_weights: Arc<RwLock<ModelWeights>>,
    challenger: Arc<RwLock<Option<AmlMlScorer>>>,
    challenger_weights: Arc<RwLock<Option<ModelWeights>>>,
}

impl ChampionChallengerFramework {
    pub fn new(
        db: PgPool,
        config: ChampionChallengerConfig,
        champion_weights: ModelWeights,
    ) -> Self {
        let scorer = AmlMlScorer::new(champion_weights.clone());
        Self {
            db,
            config,
            champion: Arc::new(RwLock::new(scorer)),
            champion_weights: Arc::new(RwLock::new(champion_weights)),
            challenger: Arc::new(RwLock::new(None)),
            challenger_weights: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a new challenger model (enters shadow mode immediately).
    pub async fn register_challenger(&self, weights: ModelWeights) {
        let scorer = AmlMlScorer::new(weights.clone());
        *self.challenger.write().await = Some(scorer);
        *self.challenger_weights.write().await = Some(weights.clone());
        info!(
            challenger_id = %weights.model_id,
            version = weights.version,
            "Challenger model registered — entering shadow mode"
        );
    }

    /// Score an alert.  Returns the champion's result (which is acted upon)
    /// and optionally the challenger's shadow result.
    ///
    /// If `challenger_traffic_fraction > 0` and this request is selected for
    /// A/B routing, the challenger result is returned as the primary result.
    pub async fn score(
        &self,
        alert_id: Uuid,
        features: &AmlFeatureVector,
    ) -> (MlScoringResult, Option<MlScoringResult>) {
        let champion_result = self.champion.read().await.score(alert_id, features);

        let challenger_result = {
            let guard = self.challenger.read().await;
            guard.as_ref().map(|c| c.score(alert_id, features))
        };

        // Persist shadow evaluation if challenger is active
        if let Some(ref cr) = challenger_result {
            let champ_weights = self.champion_weights.read().await;
            let chal_weights = self.challenger_weights.read().await;
            if let Some(ref cw) = *chal_weights {
                let _ = self
                    .persist_shadow_evaluation(
                        alert_id,
                        &champion_result,
                        cr,
                        champ_weights.model_id,
                        cw.model_id,
                    )
                    .await;
            }
        }

        // A/B routing: route a fraction of traffic to challenger
        let use_challenger = challenger_result.is_some()
            && self.config.challenger_traffic_fraction > 0.0
            && should_route_to_challenger(alert_id, self.config.challenger_traffic_fraction);

        if use_challenger {
            let cr = challenger_result.clone().unwrap();
            (cr, Some(champion_result))
        } else {
            (champion_result, challenger_result)
        }
    }

    /// Evaluate whether the challenger should be promoted to champion.
    pub async fn evaluate_promotion(&self) -> Result<PromotionDecision, sqlx::Error> {
        let chal_weights = self.challenger_weights.read().await;
        let Some(ref cw) = *chal_weights else {
            return Ok(PromotionDecision::Rejected {
                reason: "No challenger registered".into(),
                challenger_fp_rate: 0.0,
                champion_fp_rate: 0.0,
            });
        };

        // Count shadow evaluations with analyst feedback
        let stats = sqlx::query!(
            r#"
            SELECT
                COUNT(*) AS total,
                SUM(CASE WHEN analyst_confirmed_fp = true
                         AND challenger_recommendation = 'Suppress' THEN 1 ELSE 0 END) AS chal_tp,
                SUM(CASE WHEN analyst_confirmed_fp = false
                         AND challenger_recommendation = 'Suppress' THEN 1 ELSE 0 END) AS chal_fp,
                SUM(CASE WHEN analyst_confirmed_fp = true
                         AND champion_recommendation = 'Suppress' THEN 1 ELSE 0 END) AS champ_tp,
                SUM(CASE WHEN analyst_confirmed_fp = false
                         AND champion_recommendation = 'Suppress' THEN 1 ELSE 0 END) AS champ_fp
            FROM aml_shadow_evaluations
            WHERE challenger_model_id = $1
              AND analyst_confirmed_fp IS NOT NULL
            "#,
            cw.model_id,
        )
        .fetch_one(&self.db)
        .await?;

        let total = stats.total.unwrap_or(0) as u64;
        if total < self.config.min_shadow_evaluations {
            return Ok(PromotionDecision::InsufficientData {
                shadow_evaluations: total,
                required: self.config.min_shadow_evaluations,
            });
        }

        let chal_fp = stats.chal_fp.unwrap_or(0) as f64;
        let chal_tp = stats.chal_tp.unwrap_or(0) as f64;
        let champ_fp = stats.champ_fp.unwrap_or(0) as f64;
        let champ_tp = stats.champ_tp.unwrap_or(0) as f64;

        let chal_fp_rate = chal_fp / (chal_fp + chal_tp + 1e-9);
        let champ_fp_rate = champ_fp / (champ_fp + champ_tp + 1e-9);

        let improvement = if champ_fp_rate > 0.0 {
            (champ_fp_rate - chal_fp_rate) / champ_fp_rate
        } else {
            0.0
        };

        if improvement >= self.config.required_fp_improvement {
            // Promote challenger
            let old_champion_id = self.champion_weights.read().await.model_id;
            self.promote_challenger().await?;

            info!(
                challenger_id = %cw.model_id,
                improvement_pct = %format!("{:.1}%", improvement * 100.0),
                "Challenger promoted to champion"
            );

            Ok(PromotionDecision::Promoted {
                new_champion_id: cw.model_id,
                old_champion_id,
                challenger_fp_rate: chal_fp_rate,
                champion_fp_rate: champ_fp_rate,
                improvement_pct: improvement * 100.0,
            })
        } else {
            warn!(
                improvement_pct = %format!("{:.1}%", improvement * 100.0),
                required_pct = %format!("{:.1}%", self.config.required_fp_improvement * 100.0),
                "Challenger did not meet promotion threshold"
            );
            Ok(PromotionDecision::Rejected {
                reason: format!(
                    "FP improvement {:.1}% < required {:.1}%",
                    improvement * 100.0,
                    self.config.required_fp_improvement * 100.0
                ),
                challenger_fp_rate: chal_fp_rate,
                champion_fp_rate: champ_fp_rate,
            })
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    async fn promote_challenger(&self) -> Result<(), sqlx::Error> {
        let chal_weights = self.challenger_weights.read().await.clone();
        let Some(cw) = chal_weights else { return Ok(()); };

        // Demote current champion in DB
        sqlx::query!(
            "UPDATE aml_model_versions SET is_champion = false WHERE is_champion = true"
        )
        .execute(&self.db)
        .await?;

        // Promote challenger in DB
        sqlx::query!(
            "UPDATE aml_model_versions SET is_champion = true WHERE model_id = $1",
            cw.model_id
        )
        .execute(&self.db)
        .await?;

        // Swap in-memory
        let new_scorer = AmlMlScorer::new(cw.clone());
        *self.champion.write().await = new_scorer;
        *self.champion_weights.write().await = cw;
        *self.challenger.write().await = None;
        *self.challenger_weights.write().await = None;

        Ok(())
    }

    async fn persist_shadow_evaluation(
        &self,
        alert_id: Uuid,
        champion: &MlScoringResult,
        challenger: &MlScoringResult,
        champion_model_id: Uuid,
        challenger_model_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO aml_shadow_evaluations
                (eval_id, alert_id, champion_model_id, challenger_model_id,
                 champion_fp_probability, challenger_fp_probability,
                 champion_recommendation, challenger_recommendation, evaluated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (alert_id, challenger_model_id) DO NOTHING
            "#,
            Uuid::new_v4(),
            alert_id,
            champion_model_id,
            challenger_model_id,
            champion.fp_probability,
            challenger.fp_probability,
            format!("{:?}", champion.recommendation),
            format!("{:?}", challenger.recommendation),
            Utc::now(),
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

/// Deterministic routing: hash the alert_id to decide if it goes to challenger.
fn should_route_to_challenger(alert_id: Uuid, fraction: f64) -> bool {
    let bytes = alert_id.as_bytes();
    let hash = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    (hash as f64 / u32::MAX as f64) < fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_respects_fraction() {
        let ids: Vec<Uuid> = (0..1000).map(|_| Uuid::new_v4()).collect();
        let routed = ids
            .iter()
            .filter(|id| should_route_to_challenger(**id, 0.1))
            .count();
        // Should be roughly 10% ± 5%
        assert!(routed < 150, "Too many routed: {routed}");
        assert!(routed > 50, "Too few routed: {routed}");
    }

    #[test]
    fn zero_fraction_routes_none() {
        for _ in 0..100 {
            assert!(!should_route_to_challenger(Uuid::new_v4(), 0.0));
        }
    }

    #[test]
    fn full_fraction_routes_all() {
        for _ in 0..100 {
            assert!(should_route_to_challenger(Uuid::new_v4(), 1.0));
        }
    }
}
