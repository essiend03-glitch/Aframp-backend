//! AML Drift Detection — PSI-based Feature Drift & Accuracy Degradation Alerts
//!
//! Monitors two types of model degradation:
//!
//! 1. **Feature drift** (data drift): Population Stability Index (PSI) per
//!    feature.  PSI > 0.2 triggers a warning; PSI > 0.25 triggers a critical
//!    alert to the compliance team.
//!
//! 2. **Accuracy degradation**: rolling precision/recall over a recent window
//!    compared to the model's validation-set baseline.  A drop of ≥10 pp
//!    triggers an alert.

use super::ml_models::AmlFeatureVector;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// PSI thresholds (industry standard)
// ---------------------------------------------------------------------------

/// PSI < 0.1 → no significant change
pub const PSI_STABLE: f64 = 0.1;
/// 0.1 ≤ PSI < 0.2 → moderate shift, monitor
pub const PSI_WARNING: f64 = 0.2;
/// PSI ≥ 0.2 → significant shift, alert
pub const PSI_CRITICAL: f64 = 0.25;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDriftReport {
    pub feature_name: String,
    pub psi: f64,
    pub severity: DriftSeverity,
    pub baseline_distribution: Vec<f64>,
    pub current_distribution: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftSeverity {
    Stable,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyDriftReport {
    pub model_id: Uuid,
    pub baseline_precision: f64,
    pub current_precision: f64,
    pub baseline_recall: f64,
    pub current_recall: f64,
    pub precision_drop: f64,
    pub recall_drop: f64,
    pub is_degraded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftCheckResult {
    pub model_id: Uuid,
    pub checked_at: DateTime<Utc>,
    pub feature_reports: Vec<FeatureDriftReport>,
    pub accuracy_report: AccuracyDriftReport,
    /// True if any feature or accuracy alert was triggered
    pub alert_triggered: bool,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectionConfig {
    /// Number of bins for PSI histogram
    pub psi_bins: usize,
    /// Number of recent evaluations to use for accuracy drift check
    pub accuracy_window: u64,
    /// Precision drop (absolute) that triggers an accuracy alert
    pub precision_drop_threshold: f64,
    /// Recall drop (absolute) that triggers an accuracy alert
    pub recall_drop_threshold: f64,
}

impl Default for DriftDetectionConfig {
    fn default() -> Self {
        Self {
            psi_bins: 10,
            accuracy_window: 500,
            precision_drop_threshold: 0.10,
            recall_drop_threshold: 0.10,
        }
    }
}

// ---------------------------------------------------------------------------
// Detector
// ---------------------------------------------------------------------------

pub struct AmlDriftDetector {
    db: PgPool,
    config: DriftDetectionConfig,
}

impl AmlDriftDetector {
    pub fn new(db: PgPool, config: DriftDetectionConfig) -> Self {
        Self { db, config }
    }

    /// Run a full drift check for the given model.
    ///
    /// `baseline_features` — representative sample from training time.
    /// `current_features`  — recent live-traffic sample.
    pub async fn check_drift(
        &self,
        model_id: Uuid,
        baseline_precision: f64,
        baseline_recall: f64,
        baseline_features: &[AmlFeatureVector],
        current_features: &[AmlFeatureVector],
    ) -> DriftCheckResult {
        // 1. Feature drift via PSI
        let feature_reports = self.compute_feature_psi(baseline_features, current_features);

        // 2. Accuracy drift from DB
        let accuracy_report = self
            .compute_accuracy_drift(model_id, baseline_precision, baseline_recall)
            .await;

        let alert_triggered = feature_reports
            .iter()
            .any(|r| r.severity == DriftSeverity::Critical)
            || accuracy_report.is_degraded;

        let summary = build_summary(&feature_reports, &accuracy_report);

        if alert_triggered {
            warn!(
                model_id = %model_id,
                summary = %summary,
                "AML model drift alert triggered"
            );
        } else {
            info!(model_id = %model_id, "Drift check passed — model stable");
        }

        // Persist to DB
        let _ = self
            .persist_drift_metrics(model_id, &feature_reports, &accuracy_report)
            .await;

        DriftCheckResult {
            model_id,
            checked_at: Utc::now(),
            feature_reports,
            accuracy_report,
            alert_triggered,
            summary,
        }
    }

    // -----------------------------------------------------------------------
    // PSI computation
    // -----------------------------------------------------------------------

    fn compute_feature_psi(
        &self,
        baseline: &[AmlFeatureVector],
        current: &[AmlFeatureVector],
    ) -> Vec<FeatureDriftReport> {
        let names = AmlFeatureVector::FEATURE_NAMES;
        let extract: Vec<Box<dyn Fn(&AmlFeatureVector) -> f64>> = vec![
            Box::new(|f| f.velocity_24h),
            Box::new(|f| f.velocity_7d),
            Box::new(|f| f.amount_ratio_30d.min(5.0) / 5.0), // cap at 5x for binning
            Box::new(|f| f.counterparty_diversity),
            Box::new(|f| f.known_counterparty_ratio),
            Box::new(|f| f.kyc_tier_score),
            Box::new(|f| f.account_age_score),
            Box::new(|f| f.historical_fp_rate),
            Box::new(|f| f.geo_consistency),
            Box::new(|f| f.corridor_risk),
        ];

        names
            .iter()
            .zip(extract.iter())
            .map(|(name, extractor)| {
                let base_vals: Vec<f64> = baseline.iter().map(|f| extractor(f)).collect();
                let curr_vals: Vec<f64> = current.iter().map(|f| extractor(f)).collect();

                let base_dist = histogram(&base_vals, self.config.psi_bins);
                let curr_dist = histogram(&curr_vals, self.config.psi_bins);
                let psi = compute_psi(&base_dist, &curr_dist);

                let severity = if psi >= PSI_CRITICAL {
                    DriftSeverity::Critical
                } else if psi >= PSI_WARNING {
                    DriftSeverity::Warning
                } else {
                    DriftSeverity::Stable
                };

                FeatureDriftReport {
                    feature_name: name.to_string(),
                    psi,
                    severity,
                    baseline_distribution: base_dist,
                    current_distribution: curr_dist,
                }
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Accuracy drift from DB
    // -----------------------------------------------------------------------

    async fn compute_accuracy_drift(
        &self,
        model_id: Uuid,
        baseline_precision: f64,
        baseline_recall: f64,
    ) -> AccuracyDriftReport {
        // Pull recent shadow evaluations with analyst feedback
        let result = sqlx::query!(
            r#"
            SELECT
                SUM(CASE WHEN analyst_confirmed_fp = true
                         AND champion_recommendation = 'Suppress' THEN 1 ELSE 0 END) AS tp,
                SUM(CASE WHEN analyst_confirmed_fp = false
                         AND champion_recommendation = 'Suppress' THEN 1 ELSE 0 END) AS fp,
                SUM(CASE WHEN analyst_confirmed_fp = true
                         AND champion_recommendation != 'Suppress' THEN 1 ELSE 0 END) AS fn_count
            FROM (
                SELECT analyst_confirmed_fp, champion_recommendation
                FROM aml_shadow_evaluations
                WHERE champion_model_id = $1
                  AND analyst_confirmed_fp IS NOT NULL
                ORDER BY evaluated_at DESC
                LIMIT $2
            ) recent
            "#,
            model_id,
            self.config.accuracy_window as i64,
        )
        .fetch_one(&self.db)
        .await;

        let (current_precision, current_recall) = match result {
            Ok(r) => {
                let tp = r.tp.unwrap_or(0) as f64;
                let fp = r.fp.unwrap_or(0) as f64;
                let fn_ = r.fn_count.unwrap_or(0) as f64;
                let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { baseline_precision };
                let recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { baseline_recall };
                (precision, recall)
            }
            Err(_) => (baseline_precision, baseline_recall),
        };

        let precision_drop = (baseline_precision - current_precision).max(0.0);
        let recall_drop = (baseline_recall - current_recall).max(0.0);
        let is_degraded = precision_drop >= self.config.precision_drop_threshold
            || recall_drop >= self.config.recall_drop_threshold;

        AccuracyDriftReport {
            model_id,
            baseline_precision,
            current_precision,
            baseline_recall,
            current_recall,
            precision_drop,
            recall_drop,
            is_degraded,
        }
    }

    async fn persist_drift_metrics(
        &self,
        model_id: Uuid,
        feature_reports: &[FeatureDriftReport],
        accuracy: &AccuracyDriftReport,
    ) -> Result<(), sqlx::Error> {
        let max_psi = feature_reports
            .iter()
            .map(|r| r.psi)
            .fold(0.0f64, f64::max);
        let critical_features: Vec<&str> = feature_reports
            .iter()
            .filter(|r| r.severity == DriftSeverity::Critical)
            .map(|r| r.feature_name.as_str())
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO aml_drift_metrics
                (metric_id, model_id, checked_at, max_psi, critical_features_json,
                 current_precision, current_recall, precision_drop, recall_drop,
                 alert_triggered)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            Uuid::new_v4(),
            model_id,
            Utc::now(),
            max_psi,
            serde_json::to_value(&critical_features).unwrap(),
            accuracy.current_precision,
            accuracy.current_recall,
            accuracy.precision_drop,
            accuracy.recall_drop,
            accuracy.is_degraded || !critical_features.is_empty(),
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pure math helpers
// ---------------------------------------------------------------------------

/// Build a normalised histogram over [0, 1] with `bins` equal-width buckets.
fn histogram(values: &[f64], bins: usize) -> Vec<f64> {
    let mut counts = vec![0usize; bins];
    for &v in values {
        let idx = ((v.clamp(0.0, 1.0 - 1e-9)) * bins as f64) as usize;
        counts[idx] += 1;
    }
    let n = values.len().max(1) as f64;
    counts.iter().map(|&c| (c as f64 / n).max(1e-6)).collect()
}

/// Population Stability Index: PSI = Σ (actual% - expected%) * ln(actual% / expected%)
fn compute_psi(baseline: &[f64], current: &[f64]) -> f64 {
    baseline
        .iter()
        .zip(current.iter())
        .map(|(b, c)| (c - b) * (c / b).ln())
        .sum()
}

fn build_summary(features: &[FeatureDriftReport], accuracy: &AccuracyDriftReport) -> String {
    let critical: Vec<&str> = features
        .iter()
        .filter(|r| r.severity == DriftSeverity::Critical)
        .map(|r| r.feature_name.as_str())
        .collect();

    let mut parts = Vec::new();
    if !critical.is_empty() {
        parts.push(format!("Critical feature drift: {}", critical.join(", ")));
    }
    if accuracy.is_degraded {
        parts.push(format!(
            "Accuracy degraded — precision drop={:.1}pp, recall drop={:.1}pp",
            accuracy.precision_drop * 100.0,
            accuracy.recall_drop * 100.0
        ));
    }
    if parts.is_empty() {
        "All features stable; accuracy within baseline.".into()
    } else {
        parts.join(". ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psi_identical_distributions_is_zero() {
        let dist = vec![0.1, 0.2, 0.3, 0.2, 0.1, 0.05, 0.05];
        let psi = compute_psi(&dist, &dist);
        assert!(psi.abs() < 1e-9, "PSI of identical distributions should be ~0");
    }

    #[test]
    fn psi_very_different_distributions_is_high() {
        let base = vec![0.5, 0.3, 0.1, 0.05, 0.05];
        let curr = vec![0.05, 0.05, 0.1, 0.3, 0.5];
        let psi = compute_psi(&base, &curr);
        assert!(psi > PSI_CRITICAL, "PSI should be critical for reversed distribution");
    }

    #[test]
    fn histogram_sums_to_one() {
        let values: Vec<f64> = (0..100).map(|i| i as f64 / 100.0).collect();
        let hist = histogram(&values, 10);
        let sum: f64 = hist.iter().sum();
        assert!((sum - 1.0).abs() < 0.01, "Histogram should sum to ~1.0");
    }

    #[test]
    fn severity_thresholds() {
        assert_eq!(
            if 0.05 >= PSI_CRITICAL { DriftSeverity::Critical }
            else if 0.05 >= PSI_WARNING { DriftSeverity::Warning }
            else { DriftSeverity::Stable },
            DriftSeverity::Stable
        );
        assert_eq!(
            if 0.15 >= PSI_CRITICAL { DriftSeverity::Critical }
            else if 0.15 >= PSI_WARNING { DriftSeverity::Warning }
            else { DriftSeverity::Stable },
            DriftSeverity::Warning
        );
        assert_eq!(
            if 0.30 >= PSI_CRITICAL { DriftSeverity::Critical }
            else if 0.30 >= PSI_WARNING { DriftSeverity::Warning }
            else { DriftSeverity::Stable },
            DriftSeverity::Critical
        );
    }
}
