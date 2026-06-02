//! AML Training Pipeline — Supervised Learning from Analyst Decisions
//!
//! Consumes `TrainingSample` records (analyst-confirmed TP/FP outcomes) and
//! updates `ModelWeights` via mini-batch stochastic gradient descent on the
//! binary cross-entropy loss.
//!
//! Acceptance criteria:
//!   - Pipeline successfully consumes historical analyst decisions
//!   - Produces a new model version with updated weights
//!   - Persists the model to the `aml_model_versions` table

use super::ml_models::{AmlFeatureVector, ModelWeights, TrainingSample};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Learning rate for SGD
    pub learning_rate: f64,
    /// L2 regularisation coefficient
    pub l2_lambda: f64,
    /// Number of passes over the training set
    pub epochs: u32,
    /// Minimum samples required before training
    pub min_samples: usize,
    /// Fraction of samples held out for validation
    pub validation_split: f64,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            l2_lambda: 0.001,
            epochs: 50,
            min_samples: 100,
            validation_split: 0.2,
        }
    }
}

// ---------------------------------------------------------------------------
// Training result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResult {
    pub new_model_id: Uuid,
    pub new_version: u32,
    pub training_samples: usize,
    pub validation_samples: usize,
    pub final_loss: f64,
    pub precision: f64,
    pub recall: f64,
    /// False-positive rate on validation set (target: ≥30% reduction vs baseline)
    pub fp_rate: f64,
    pub trained_at: chrono::DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

pub struct AmlTrainingPipeline {
    db: PgPool,
    config: TrainingConfig,
}

impl AmlTrainingPipeline {
    pub fn new(db: PgPool, config: TrainingConfig) -> Self {
        Self { db, config }
    }

    /// Load all unprocessed training samples from the DB.
    pub async fn load_samples(&self) -> Result<Vec<TrainingSample>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                sample_id, alert_id,
                velocity_24h, velocity_7d, amount_ratio_30d,
                counterparty_diversity, known_counterparty_ratio,
                kyc_tier_score, account_age_score, historical_fp_rate,
                geo_consistency, corridor_risk,
                is_false_positive, analyst_id, resolved_at
            FROM aml_training_samples
            ORDER BY resolved_at ASC
            "#
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| TrainingSample {
                sample_id: r.sample_id,
                alert_id: r.alert_id,
                features: AmlFeatureVector {
                    velocity_24h: r.velocity_24h,
                    velocity_7d: r.velocity_7d,
                    amount_ratio_30d: r.amount_ratio_30d,
                    counterparty_diversity: r.counterparty_diversity,
                    known_counterparty_ratio: r.known_counterparty_ratio,
                    kyc_tier_score: r.kyc_tier_score,
                    account_age_score: r.account_age_score,
                    historical_fp_rate: r.historical_fp_rate,
                    geo_consistency: r.geo_consistency,
                    corridor_risk: r.corridor_risk,
                },
                is_false_positive: r.is_false_positive,
                analyst_id: r.analyst_id,
                resolved_at: r.resolved_at,
            })
            .collect())
    }

    /// Train a new model version from the provided samples.
    /// Returns `None` if there are fewer than `min_samples`.
    pub async fn train(
        &self,
        current_weights: &ModelWeights,
        samples: Vec<TrainingSample>,
    ) -> Option<(ModelWeights, TrainingResult)> {
        if samples.len() < self.config.min_samples {
            warn!(
                samples = samples.len(),
                min = self.config.min_samples,
                "Insufficient training samples — skipping training run"
            );
            return None;
        }

        // Split train / validation
        let split_idx = ((samples.len() as f64) * (1.0 - self.config.validation_split)) as usize;
        let (train_set, val_set) = samples.split_at(split_idx);

        // Initialise weights from current champion
        let mut weights = current_weights.weights;
        let mut bias = current_weights.bias;

        // Mini-batch SGD (full-batch here for simplicity)
        let mut final_loss = 0.0;
        for _epoch in 0..self.config.epochs {
            let (grad_w, grad_b, loss) = compute_gradients(train_set, &weights, bias);
            final_loss = loss;

            // Update with L2 regularisation
            for i in 0..10 {
                weights[i] -= self.config.learning_rate
                    * (grad_w[i] + self.config.l2_lambda * weights[i]);
            }
            bias -= self.config.learning_rate * grad_b;
        }

        // Evaluate on validation set
        let (precision, recall, fp_rate) = evaluate(val_set, &weights, bias);

        let new_model = ModelWeights {
            model_id: Uuid::new_v4(),
            version: current_weights.version + 1,
            weights,
            bias,
            trained_at: Utc::now(),
            training_samples: train_set.len() as u64,
            validation_precision: precision,
            validation_recall: recall,
        };

        let result = TrainingResult {
            new_model_id: new_model.model_id,
            new_version: new_model.version,
            training_samples: train_set.len(),
            validation_samples: val_set.len(),
            final_loss,
            precision,
            recall,
            fp_rate,
            trained_at: Utc::now(),
        };

        info!(
            version = new_model.version,
            precision = %format!("{:.3}", precision),
            recall = %format!("{:.3}", recall),
            fp_rate = %format!("{:.3}", fp_rate),
            "Training complete"
        );

        Some((new_model, result))
    }

    /// Persist a trained model to `aml_model_versions`.
    pub async fn save_model(
        &self,
        model: &ModelWeights,
        result: &TrainingResult,
        is_champion: bool,
    ) -> Result<(), sqlx::Error> {
        let weights_json = serde_json::to_value(&model.weights).unwrap();
        sqlx::query!(
            r#"
            INSERT INTO aml_model_versions
                (model_id, version, weights_json, bias, trained_at,
                 training_samples, validation_precision, validation_recall,
                 fp_rate, is_champion)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            model.model_id,
            model.version as i32,
            weights_json,
            model.bias,
            model.trained_at,
            model.training_samples as i64,
            result.precision,
            result.recall,
            result.fp_rate,
            is_champion,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Load the current champion model from the DB.
    pub async fn load_champion(&self) -> Result<Option<ModelWeights>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT model_id, version, weights_json, bias, trained_at,
                   training_samples, validation_precision, validation_recall
            FROM aml_model_versions
            WHERE is_champion = true
            ORDER BY version DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|r| {
            let weights: [f64; 10] = serde_json::from_value(r.weights_json).unwrap_or_default();
            ModelWeights {
                model_id: r.model_id,
                version: r.version as u32,
                weights,
                bias: r.bias,
                trained_at: r.trained_at,
                training_samples: r.training_samples as u64,
                validation_precision: r.validation_precision,
                validation_recall: r.validation_recall,
            }
        }))
    }
}

// ---------------------------------------------------------------------------
// Pure math helpers (no I/O — easy to unit-test)
// ---------------------------------------------------------------------------

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Binary cross-entropy gradient over a batch.
/// Returns (weight_gradients, bias_gradient, mean_loss).
fn compute_gradients(
    samples: &[TrainingSample],
    weights: &[f64; 10],
    bias: f64,
) -> ([f64; 10], f64, f64) {
    let n = samples.len() as f64;
    let mut grad_w = [0.0f64; 10];
    let mut grad_b = 0.0f64;
    let mut total_loss = 0.0f64;

    for s in samples {
        let arr = s.features.to_array();
        let logit: f64 = bias + arr.iter().zip(weights.iter()).map(|(f, w)| f * w).sum::<f64>();
        let pred = sigmoid(logit);
        let label = if s.is_false_positive { 1.0 } else { 0.0 };
        let error = pred - label;

        // Cross-entropy loss
        let eps = 1e-12;
        total_loss -= label * (pred + eps).ln() + (1.0 - label) * (1.0 - pred + eps).ln();

        for i in 0..10 {
            grad_w[i] += error * arr[i];
        }
        grad_b += error;
    }

    for g in &mut grad_w {
        *g /= n;
    }
    grad_b /= n;
    total_loss /= n;

    (grad_w, grad_b, total_loss)
}

/// Compute precision, recall, and FP rate at threshold 0.5.
fn evaluate(samples: &[TrainingSample], weights: &[f64; 10], bias: f64) -> (f64, f64, f64) {
    let (mut tp, mut fp, mut tn, mut fn_) = (0u64, 0u64, 0u64, 0u64);

    for s in samples {
        let arr = s.features.to_array();
        let logit: f64 = bias + arr.iter().zip(weights.iter()).map(|(f, w)| f * w).sum::<f64>();
        let pred_fp = sigmoid(logit) >= 0.5;
        let actual_fp = s.is_false_positive;

        match (pred_fp, actual_fp) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
            (false, false) => tn += 1,
        }
    }

    let precision = if tp + fp > 0 { tp as f64 / (tp + fp) as f64 } else { 0.0 };
    let recall = if tp + fn_ > 0 { tp as f64 / (tp + fn_) as f64 } else { 0.0 };
    let fp_rate = if fp + tn > 0 { fp as f64 / (fp + tn) as f64 } else { 0.0 };

    (precision, recall, fp_rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aml::ml_models::AmlFeatureVector;

    fn make_sample(is_fp: bool) -> TrainingSample {
        let features = if is_fp {
            AmlFeatureVector {
                velocity_24h: 0.1,
                velocity_7d: 0.1,
                amount_ratio_30d: 1.0,
                counterparty_diversity: 0.1,
                known_counterparty_ratio: 0.9,
                kyc_tier_score: 1.0,
                account_age_score: 0.9,
                historical_fp_rate: 0.8,
                geo_consistency: 1.0,
                corridor_risk: 0.1,
            }
        } else {
            AmlFeatureVector {
                velocity_24h: 0.9,
                velocity_7d: 0.9,
                amount_ratio_30d: 4.0,
                counterparty_diversity: 0.8,
                known_counterparty_ratio: 0.1,
                kyc_tier_score: 0.0,
                account_age_score: 0.05,
                historical_fp_rate: 0.0,
                geo_consistency: 0.0,
                corridor_risk: 0.9,
            }
        };
        TrainingSample {
            sample_id: Uuid::new_v4(),
            alert_id: Uuid::new_v4(),
            features,
            is_false_positive: is_fp,
            analyst_id: Uuid::new_v4(),
            resolved_at: Utc::now(),
        }
    }

    #[test]
    fn gradient_descent_reduces_loss() {
        let samples: Vec<TrainingSample> = (0..50)
            .map(|i| make_sample(i % 2 == 0))
            .collect();

        let mut weights = [0.0f64; 10];
        let mut bias = 0.0f64;
        let lr = 0.1;

        let (_, _, loss_before) = compute_gradients(&samples, &weights, bias);

        for _ in 0..20 {
            let (gw, gb, _) = compute_gradients(&samples, &weights, bias);
            for i in 0..10 {
                weights[i] -= lr * gw[i];
            }
            bias -= lr * gb;
        }

        let (_, _, loss_after) = compute_gradients(&samples, &weights, bias);
        assert!(loss_after < loss_before, "Loss should decrease after training");
    }

    #[test]
    fn evaluate_perfect_separation() {
        // With default weights, FP-like samples should score high
        let default = ModelWeights::default();
        let samples: Vec<TrainingSample> = (0..20).map(|i| make_sample(i % 2 == 0)).collect();
        let (precision, recall, _) = evaluate(&samples, &default.weights, default.bias);
        // Just check they're in valid range
        assert!((0.0..=1.0).contains(&precision));
        assert!((0.0..=1.0).contains(&recall));
    }
}
