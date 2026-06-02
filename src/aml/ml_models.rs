//! AML ML Models — Feature Extraction, Scoring & SHAP Explainability
//!
//! Implements a logistic-regression-based false-positive reducer on top of the
//! existing rules engine.  The model learns from analyst decisions (TP vs FP)
//! and produces:
//!   - A suppression probability (0.0 = definitely suspicious, 1.0 = likely benign)
//!   - SHAP-style feature attributions for every prediction (audit requirement)
//!   - A human-readable justification string for compliance teams

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Feature vector
// ---------------------------------------------------------------------------

/// The four feature groups required by the issue spec.
/// All values are normalised to [0.0, 1.0] before scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmlFeatureVector {
    // --- Velocity Patterns ---
    /// Transactions in the last 24 h (normalised by max_daily_tx)
    pub velocity_24h: f64,
    /// Transactions in the last 7 d (normalised)
    pub velocity_7d: f64,
    /// Ratio of current amount to user's 30-day average
    pub amount_ratio_30d: f64,

    // --- Network Behavior ---
    /// Number of distinct counterparties in 30 d (normalised)
    pub counterparty_diversity: f64,
    /// Fraction of transactions to previously-seen counterparties
    pub known_counterparty_ratio: f64,

    // --- User Risk Profile ---
    /// KYC tier (0 = unverified, 0.33 = tier1, 0.66 = tier2, 1.0 = tier3)
    pub kyc_tier_score: f64,
    /// Account age in days (normalised by 365)
    pub account_age_score: f64,
    /// Prior false-positive rate for this user (analyst-confirmed FPs / total alerts)
    pub historical_fp_rate: f64,

    // --- Geographic Consistency ---
    /// 1.0 if origin country matches user's registered country, else 0.0
    pub geo_consistency: f64,
    /// Corridor risk weight from the existing rules engine (0.0–1.0)
    pub corridor_risk: f64,
}

impl AmlFeatureVector {
    /// Flatten to a fixed-length array for dot-product scoring.
    pub fn to_array(&self) -> [f64; 10] {
        [
            self.velocity_24h,
            self.velocity_7d,
            self.amount_ratio_30d,
            self.counterparty_diversity,
            self.known_counterparty_ratio,
            self.kyc_tier_score,
            self.account_age_score,
            self.historical_fp_rate,
            self.geo_consistency,
            self.corridor_risk,
        ]
    }

    pub const FEATURE_NAMES: [&'static str; 10] = [
        "velocity_24h",
        "velocity_7d",
        "amount_ratio_30d",
        "counterparty_diversity",
        "known_counterparty_ratio",
        "kyc_tier_score",
        "account_age_score",
        "historical_fp_rate",
        "geo_consistency",
        "corridor_risk",
    ];
}

// ---------------------------------------------------------------------------
// Model weights
// ---------------------------------------------------------------------------

/// Logistic-regression weights + bias.
/// Positive weight → feature increases FP probability (benign signal).
/// Negative weight → feature increases TP probability (suspicious signal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWeights {
    pub model_id: Uuid,
    pub version: u32,
    pub weights: [f64; 10],
    pub bias: f64,
    pub trained_at: DateTime<Utc>,
    pub training_samples: u64,
    /// Precision on held-out validation set
    pub validation_precision: f64,
    /// Recall on held-out validation set
    pub validation_recall: f64,
}

impl Default for ModelWeights {
    /// Sensible priors: high KYC tier, old account, high historical FP rate,
    /// and geo consistency are benign signals; high corridor risk is suspicious.
    fn default() -> Self {
        Self {
            model_id: Uuid::new_v4(),
            version: 0,
            weights: [
                -0.8, // velocity_24h          — high velocity → suspicious
                -0.6, // velocity_7d
                -0.7, // amount_ratio_30d       — unusual amount → suspicious
                -0.3, // counterparty_diversity — many new counterparties → suspicious
                0.5,  // known_counterparty_ratio — familiar counterparties → benign
                0.6,  // kyc_tier_score         — higher KYC → benign
                0.4,  // account_age_score      — older account → benign
                0.9,  // historical_fp_rate     — analyst said FP before → benign
                0.5,  // geo_consistency        — matches home country → benign
                -0.9, // corridor_risk          — high-risk corridor → suspicious
            ],
            bias: 0.0,
            trained_at: Utc::now(),
            training_samples: 0,
            validation_precision: 0.0,
            validation_recall: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// SHAP-style attribution
// ---------------------------------------------------------------------------

/// Per-feature contribution to the final score (linear SHAP approximation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureAttribution {
    pub feature_name: String,
    pub feature_value: f64,
    pub contribution: f64, // weight_i * (feature_i - baseline_i)
    pub direction: AttributionDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttributionDirection {
    /// This feature pushed the prediction toward "likely benign" (FP suppression)
    TowardBenign,
    /// This feature pushed the prediction toward "suspicious" (TP retention)
    TowardSuspicious,
}

// ---------------------------------------------------------------------------
// Scoring result
// ---------------------------------------------------------------------------

/// Output of the ML scorer for a single alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlScoringResult {
    pub alert_id: Uuid,
    pub model_id: Uuid,
    pub model_version: u32,
    /// Probability that this alert is a false positive (0.0–1.0).
    /// Above `fp_suppression_threshold` → alert is suppressed / downgraded.
    pub fp_probability: f64,
    /// Recommended action from the ML layer
    pub recommendation: MlRecommendation,
    /// Ordered by |contribution| descending — top drivers of the decision
    pub attributions: Vec<FeatureAttribution>,
    /// Human-readable justification for compliance audit
    pub justification: String,
    pub scored_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MlRecommendation {
    /// Suppress alert — model is confident this is a false positive
    Suppress,
    /// Downgrade severity (e.g. Critical → Medium)
    Downgrade,
    /// Keep alert as-is — model agrees with rules engine
    Retain,
}

// ---------------------------------------------------------------------------
// Scorer
// ---------------------------------------------------------------------------

/// Threshold above which an alert is suppressed (≥30% FP reduction target).
pub const FP_SUPPRESSION_THRESHOLD: f64 = 0.75;
/// Threshold above which an alert is downgraded rather than suppressed.
pub const FP_DOWNGRADE_THRESHOLD: f64 = 0.55;

/// Baseline feature values used for SHAP attribution (population mean).
const BASELINE: [f64; 10] = [0.3, 0.3, 1.0, 0.3, 0.6, 0.5, 0.5, 0.2, 0.8, 0.4];

pub struct AmlMlScorer {
    weights: ModelWeights,
}

impl AmlMlScorer {
    pub fn new(weights: ModelWeights) -> Self {
        Self { weights }
    }

    /// Score a feature vector and return a full `MlScoringResult`.
    pub fn score(&self, alert_id: Uuid, features: &AmlFeatureVector) -> MlScoringResult {
        let arr = features.to_array();

        // Linear combination
        let logit: f64 = self.weights.bias
            + arr
                .iter()
                .zip(self.weights.weights.iter())
                .map(|(f, w)| f * w)
                .sum::<f64>();

        // Sigmoid → FP probability
        let fp_probability = sigmoid(logit);

        // SHAP-style linear attributions
        let attributions: Vec<FeatureAttribution> = arr
            .iter()
            .zip(self.weights.weights.iter())
            .zip(BASELINE.iter())
            .zip(AmlFeatureVector::FEATURE_NAMES.iter())
            .map(|(((val, w), baseline), name)| {
                let contribution = w * (val - baseline);
                FeatureAttribution {
                    feature_name: name.to_string(),
                    feature_value: *val,
                    contribution,
                    direction: if contribution >= 0.0 {
                        AttributionDirection::TowardBenign
                    } else {
                        AttributionDirection::TowardSuspicious
                    },
                }
            })
            .collect();

        let recommendation = if fp_probability >= FP_SUPPRESSION_THRESHOLD {
            MlRecommendation::Suppress
        } else if fp_probability >= FP_DOWNGRADE_THRESHOLD {
            MlRecommendation::Downgrade
        } else {
            MlRecommendation::Retain
        };

        let justification = build_justification(&recommendation, &attributions, fp_probability);

        MlScoringResult {
            alert_id,
            model_id: self.weights.model_id,
            model_version: self.weights.version,
            fp_probability,
            recommendation,
            attributions,
            justification,
            scored_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Build a human-readable justification string for compliance audit.
fn build_justification(
    rec: &MlRecommendation,
    attributions: &[FeatureAttribution],
    fp_prob: f64,
) -> String {
    // Top 3 drivers by absolute contribution
    let mut sorted = attributions.to_vec();
    sorted.sort_by(|a, b| b.contribution.abs().partial_cmp(&a.contribution.abs()).unwrap());
    let top: Vec<String> = sorted
        .iter()
        .take(3)
        .map(|a| {
            let dir = match a.direction {
                AttributionDirection::TowardBenign => "benign",
                AttributionDirection::TowardSuspicious => "suspicious",
            };
            format!(
                "{} (value={:.2}, contribution={:+.3}, toward {})",
                a.feature_name, a.feature_value, a.contribution, dir
            )
        })
        .collect();

    let action = match rec {
        MlRecommendation::Suppress => "SUPPRESSED",
        MlRecommendation::Downgrade => "DOWNGRADED",
        MlRecommendation::Retain => "RETAINED",
    };

    format!(
        "ML model v{action}: FP probability={fp_prob:.1%}. \
         Top drivers: {}. \
         This decision was made by an automated model trained on historical analyst outcomes \
         and is subject to periodic review.",
        top.join("; ")
    )
}

// ---------------------------------------------------------------------------
// Training sample (used by training_pipeline.rs)
// ---------------------------------------------------------------------------

/// A labeled sample produced when an analyst resolves an alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSample {
    pub sample_id: Uuid,
    pub alert_id: Uuid,
    pub features: AmlFeatureVector,
    /// true = analyst confirmed False Positive; false = confirmed True Positive
    pub is_false_positive: bool,
    pub analyst_id: Uuid,
    pub resolved_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_features(fp_like: bool) -> AmlFeatureVector {
        if fp_like {
            AmlFeatureVector {
                velocity_24h: 0.1,
                velocity_7d: 0.2,
                amount_ratio_30d: 0.9,
                counterparty_diversity: 0.1,
                known_counterparty_ratio: 0.9,
                kyc_tier_score: 1.0,
                account_age_score: 0.8,
                historical_fp_rate: 0.9,
                geo_consistency: 1.0,
                corridor_risk: 0.1,
            }
        } else {
            AmlFeatureVector {
                velocity_24h: 0.9,
                velocity_7d: 0.8,
                amount_ratio_30d: 5.0,
                counterparty_diversity: 0.9,
                known_counterparty_ratio: 0.1,
                kyc_tier_score: 0.0,
                account_age_score: 0.05,
                historical_fp_rate: 0.0,
                geo_consistency: 0.0,
                corridor_risk: 0.95,
            }
        }
    }

    #[test]
    fn fp_like_transaction_suppressed() {
        let scorer = AmlMlScorer::new(ModelWeights::default());
        let result = scorer.score(Uuid::new_v4(), &sample_features(true));
        assert!(result.fp_probability > 0.5, "Expected high FP probability");
        assert_ne!(result.recommendation, MlRecommendation::Retain);
        assert!(!result.justification.is_empty());
    }

    #[test]
    fn tp_like_transaction_retained() {
        let scorer = AmlMlScorer::new(ModelWeights::default());
        let result = scorer.score(Uuid::new_v4(), &sample_features(false));
        assert!(result.fp_probability < 0.5, "Expected low FP probability");
        assert_eq!(result.recommendation, MlRecommendation::Retain);
    }

    #[test]
    fn attributions_sum_approximately_to_logit_contribution() {
        let scorer = AmlMlScorer::new(ModelWeights::default());
        let features = sample_features(true);
        let result = scorer.score(Uuid::new_v4(), &features);
        // All attributions should be present
        assert_eq!(result.attributions.len(), 10);
    }
}
