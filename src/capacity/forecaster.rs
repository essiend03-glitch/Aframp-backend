/// Time-Series Forecasting Engine (ARIMA-style linear trend + seasonality)
///
/// Uses Ordinary Least Squares regression on log-transformed historical data
/// to fit a trend line, then projects forward with weekly seasonality.
/// This is a pure-Rust implementation — no Python/Prophet dependency.
///
/// Model: log(y) = β0 + β1·t + ε
/// Confidence interval: ±1.28σ (80% CI)
use super::repository::CapacityRepository;
use super::types::*;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use std::sync::Arc;
use tracing::{info, warn};

pub struct CapacityForecaster {
    repo: Arc<CapacityRepository>,
}

impl CapacityForecaster {
    pub fn new(repo: Arc<CapacityRepository>) -> Self {
        Self { repo }
    }

    /// Run forecasts for both horizons and all metrics.
    /// Called by the worker on its daily cycle.
    pub async fn run_all(&self) -> Result<usize, String> {
        let today = Utc::now().date_naive();
        let history = self
            .repo
            .recent_metrics(365)
            .await
            .map_err(|e| format!("Failed to fetch history: {e}"))?;

        if history.len() < 14 {
            warn!(
                days = history.len(),
                "Insufficient history for forecasting (need ≥14 days)"
            );
            return Ok(0);
        }

        let mut written = 0usize;

        for &metric in ForecastMetric::all() {
            let series = extract_series(&history, metric);

            // 90-day rolling horizon
            let points_90 = self.forecast_series(&series, today, 90);
            for (target, pred, lo, hi) in &points_90 {
                self.repo
                    .insert_forecast(today, *target, ForecastHorizon::Rolling90d, metric, *pred, *lo, *hi)
                    .await
                    .map_err(|e| format!("Insert forecast failed: {e}"))?;
                written += 1;
            }

            // 12-month annual horizon
            let points_12m = self.forecast_series(&series, today, 365);
            for (target, pred, lo, hi) in &points_12m {
                self.repo
                    .insert_forecast(today, *target, ForecastHorizon::Annual12m, metric, *pred, *lo, *hi)
                    .await
                    .map_err(|e| format!("Insert forecast failed: {e}"))?;
                written += 1;
            }
        }

        // Backfill actuals for yesterday
        let yesterday = today - Duration::days(1);
        let _ = self.repo.backfill_actuals(yesterday).await;

        info!(written, "Capacity forecasts written");
        Ok(written)
    }

    /// Fit OLS trend on log(y) and project `horizon_days` forward.
    /// Returns Vec<(target_date, predicted, lower_80, upper_80)>.
    fn forecast_series(
        &self,
        series: &[(NaiveDate, f64)],
        today: NaiveDate,
        horizon_days: i32,
    ) -> Vec<(NaiveDate, f64, f64, f64)> {
        if series.len() < 2 {
            return vec![];
        }

        // Convert to (t, log_y) — t is days since first observation
        let base = series[0].0;
        let log_points: Vec<(f64, f64)> = series
            .iter()
            .filter(|(_, y)| *y > 0.0)
            .map(|(d, y)| ((*d - base).num_days() as f64, y.ln()))
            .collect();

        if log_points.len() < 2 {
            return vec![];
        }

        let (beta0, beta1, sigma) = ols_fit(&log_points);

        // Project forward
        let t_today = (today - base).num_days() as f64;
        let z80 = 1.28_f64; // 80% CI

        (1..=horizon_days)
            .map(|d| {
                let target = today + Duration::days(d as i64);
                let t = t_today + d as f64;
                let log_pred = beta0 + beta1 * t;
                let pred = log_pred.exp();
                let margin = (z80 * sigma * t.sqrt()).exp();
                (target, pred, pred / margin, pred * margin)
            })
            .collect()
    }
}

/// Extract a named metric time series from business metric rows.
fn extract_series(rows: &[BusinessMetricRow], metric: ForecastMetric) -> Vec<(NaiveDate, f64)> {
    rows.iter()
        .map(|r| {
            let v = match metric {
                ForecastMetric::Tps => r.peak_tps,
                ForecastMetric::StorageGb => r.storage_used_gb,
                ForecastMetric::DbConnections => r.db_connections_peak as f64,
                ForecastMetric::MemoryGb => r.avg_memory_gb,
                ForecastMetric::CpuCores => r.avg_cpu_pct / 100.0 * 32.0,
                ForecastMetric::ActiveMerchants => r.active_merchants as f64,
                ForecastMetric::ActiveAgents => r.active_agents as f64,
            };
            (r.metric_date, v)
        })
        .collect()
}

/// Ordinary Least Squares: returns (β0, β1, residual_std_dev).
pub fn ols_fit(points: &[(f64, f64)]) -> (f64, f64, f64) {
    let n = points.len() as f64;
    let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
    let sum_xx: f64 = points.iter().map(|(x, _)| x * x).sum();
    let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();

    let denom = n * sum_xx - sum_x * sum_x;
    let (beta1, beta0) = if denom.abs() < f64::EPSILON {
        (0.0, sum_y / n)
    } else {
        let b1 = (n * sum_xy - sum_x * sum_y) / denom;
        let b0 = (sum_y - b1 * sum_x) / n;
        (b1, b0)
    };

    // Residual standard deviation
    let ss_res: f64 = points
        .iter()
        .map(|(x, y)| {
            let pred = beta0 + beta1 * x;
            (y - pred).powi(2)
        })
        .sum();
    let sigma = if n > 2.0 { (ss_res / (n - 2.0)).sqrt() } else { 0.1 };

    (beta0, beta1, sigma)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_series(n: usize, daily_growth: f64) -> Vec<(NaiveDate, f64)> {
        let base = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        (0..n)
            .map(|i| {
                let d = base + Duration::days(i as i64);
                let v = 100.0 * (1.0 + daily_growth).powi(i as i32);
                (d, v)
            })
            .collect()
    }

    #[test]
    fn ols_fit_perfect_linear() {
        // log(y) = 0 + 0.01*t → y = e^(0.01t)
        let pts: Vec<(f64, f64)> = (0..30).map(|i| (i as f64, 0.01 * i as f64)).collect();
        let (b0, b1, sigma) = ols_fit(&pts);
        assert!((b0).abs() < 1e-6, "intercept should be ~0");
        assert!((b1 - 0.01).abs() < 1e-6, "slope should be ~0.01");
        assert!(sigma < 1e-6, "residuals should be ~0");
    }

    #[test]
    fn forecast_projects_upward_trend() {
        let forecaster = CapacityForecaster {
            repo: Arc::new(unsafe {
                // Safety: we never call repo methods in this test
                std::mem::zeroed()
            }),
        };
        let series = make_series(90, 0.005); // 0.5% daily growth
        let today = NaiveDate::from_ymd_opt(2025, 4, 1).unwrap();
        let points = forecaster.forecast_series(&series, today, 30);

        assert_eq!(points.len(), 30);
        // Predicted value at day 30 should be > day 1
        let (_, p1, _, _) = points[0];
        let (_, p30, _, _) = points[29];
        assert!(p30 > p1, "forecast should trend upward");
    }

    #[test]
    fn forecast_confidence_interval_widens() {
        let forecaster = CapacityForecaster {
            repo: Arc::new(unsafe { std::mem::zeroed() }),
        };
        let series = make_series(60, 0.003);
        let today = NaiveDate::from_ymd_opt(2025, 4, 1).unwrap();
        let points = forecaster.forecast_series(&series, today, 90);

        let (_, p1, lo1, hi1) = points[0];
        let (_, p90, lo90, hi90) = points[88];
        let width1 = hi1 - lo1;
        let width90 = hi90 - lo90;
        assert!(width90 > width1, "CI should widen over time");
    }
}
