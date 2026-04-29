/// Capacity Planning Engine — orchestrates all subsystems.
use super::forecaster::CapacityForecaster;
use super::repository::CapacityRepository;
use super::types::*;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Alert lead-time: fire at least 60 days before projected breach.
const ALERT_LEAD_DAYS: i32 = 60;
/// Storage threshold: alert when projected to exhaust within ALERT_LEAD_DAYS.
const STORAGE_CEILING_GB: f64 = 10_000.0;
/// TPS ceiling before infra team must scale.
const TPS_CEILING: f64 = 5_000.0;
/// DB connection ceiling.
const DB_CONN_CEILING: f64 = 2_000.0;
/// Memory ceiling (GB).
const MEMORY_CEILING_GB: f64 = 512.0;

pub struct CapacityEngine {
    repo: Arc<CapacityRepository>,
    forecaster: CapacityForecaster,
}

impl CapacityEngine {
    pub fn new(repo: Arc<CapacityRepository>) -> Self {
        let forecaster = CapacityForecaster::new(Arc::clone(&repo));
        Self { repo, forecaster }
    }

    pub fn repo(&self) -> &CapacityRepository {
        &self.repo
    }

    // ── Metric ingestion ──────────────────────────────────────────────────────

    pub async fn ingest_metrics(
        &self,
        req: IngestMetricsRequest,
    ) -> Result<BusinessMetricRow, String> {
        let row = self
            .repo
            .upsert_business_metrics(&req)
            .await
            .map_err(|e| format!("Ingest failed: {e}"))?;

        // After each ingest, update the RCU model if we have enough history
        if let Err(e) = self.update_rcu_model().await {
            warn!(error = %e, "RCU model update failed after ingest");
        }

        info!(metric_date = %row.metric_date, "Business metrics ingested");
        Ok(row)
    }

    // ── RCU model update (monthly) ────────────────────────────────────────────

    /// Recompute the Resource Consumption Unit model from the last 30 days of actuals.
    pub async fn update_rcu_model(&self) -> Result<ResourceConsumptionUnit, String> {
        let history = self
            .repo
            .recent_metrics(30)
            .await
            .map_err(|e| format!("History fetch failed: {e}"))?;

        if history.len() < 7 {
            return Err("Insufficient history for RCU update (need ≥7 days)".into());
        }

        let n = history.len() as f64;
        let avg_tps: f64 = history.iter().map(|r| r.peak_tps).sum::<f64>() / n;
        let avg_cpu: f64 = history.iter().map(|r| r.avg_cpu_pct / 100.0 * 32.0).sum::<f64>() / n;
        let avg_mem: f64 = history.iter().map(|r| r.avg_memory_gb).sum::<f64>() / n;
        let avg_tx: f64 = history.iter().map(|r| r.daily_transactions as f64).sum::<f64>() / n;
        let avg_storage_growth: f64 = history.iter().map(|r| r.storage_growth_gb).sum::<f64>() / n;
        let avg_agents: f64 = history.iter().map(|r| r.active_agents as f64).sum::<f64>() / n;
        let avg_merchants: f64 = history.iter().map(|r| r.active_merchants as f64).sum::<f64>() / n;
        let avg_db: f64 = history.iter().map(|r| r.db_connections_peak as f64).sum::<f64>() / n;
        let avg_api: f64 = history.iter().map(|r| r.api_call_volume as f64).sum::<f64>() / n;

        // Derive RCU coefficients from actuals
        let cpu_per_1k = if avg_tps > 0.0 { avg_cpu / (avg_tps / 1000.0) } else { 2.0 };
        let mem_per_1k = if avg_tps > 0.0 { avg_mem / (avg_tps / 1000.0) } else { 4.0 };
        let storage_per_1k = if avg_tx > 0.0 { avg_storage_growth / (avg_tx / 1000.0) } else { 0.5 };
        let db_per_agent = if avg_agents > 0.0 { avg_db * 0.6 / avg_agents } else { 0.5 };
        let db_per_merchant = if avg_merchants > 0.0 { avg_db * 0.4 / avg_merchants } else { 0.3 };
        let mem_per_api = if avg_api > 0.0 { (avg_mem * 1024.0 * 0.2) / avg_api } else { 0.01 };

        // Compute forecast accuracy from last month's forecasts vs actuals
        let accuracy = self.compute_forecast_accuracy().await.ok();

        let month = {
            let today = Utc::now().date_naive();
            NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap()
        };

        let rcu = self
            .repo
            .upsert_rcu(
                month,
                cpu_per_1k, mem_per_1k, cpu_per_1k * 100.0, // iops ~ cpu proxy
                storage_per_1k, db_per_agent, db_per_merchant,
                mem_per_api, 1.30, accuracy,
            )
            .await
            .map_err(|e| format!("RCU upsert failed: {e}"))?;

        info!(model_month = %month, "RCU model updated from actuals");
        Ok(rcu)
    }

    async fn compute_forecast_accuracy(&self) -> Result<f64, String> {
        let val: Option<f64> = sqlx::query_scalar!(
            r#"
            SELECT AVG(ape_pct)::float8
            FROM capacity_forecasts
            WHERE actual_value IS NOT NULL
              AND forecast_date >= CURRENT_DATE - INTERVAL '30 days'
            "#
        )
        .fetch_one(self.repo.db())
        .await
        .map_err(|e| e.to_string())?;
        Ok(val.unwrap_or(0.0))
    }

    // ── Forecasting ───────────────────────────────────────────────────────────

    pub async fn run_forecasts(&self) -> Result<usize, String> {
        self.forecaster.run_all().await
    }

    pub async fn get_forecasts(
        &self,
        horizon: ForecastHorizon,
    ) -> Result<Vec<CapacityForecast>, String> {
        let today = Utc::now().date_naive();
        self.repo
            .forecasts_for_horizon(horizon, today)
            .await
            .map_err(|e| format!("Forecast fetch failed: {e}"))
    }

    // ── What-If simulation ────────────────────────────────────────────────────

    pub async fn run_scenario(
        &self,
        req: RunScenarioRequest,
        created_by: &str,
    ) -> Result<CapacityScenario, String> {
        let rcu = self
            .repo
            .latest_rcu()
            .await
            .map_err(|e| format!("RCU fetch failed: {e}"))?
            .ok_or("No RCU model found — ingest metrics first")?;

        let baseline = self
            .repo
            .latest_metric()
            .await
            .map_err(|e| format!("Baseline fetch failed: {e}"))?
            .ok_or("No baseline metrics found")?;

        let provider = req.cloud_provider.as_deref().unwrap_or("aws");
        let pricing = CloudPricingConfig::from_name(provider);

        // Project business drivers after the scenario
        let projected_tps = baseline.peak_tps * req.transaction_volume_multiplier;
        let projected_merchants = baseline.active_merchants + req.new_merchant_chains * 50;
        let projected_agents = baseline.active_agents + req.new_agent_count;
        let projected_tx = (baseline.daily_transactions as f64 * req.transaction_volume_multiplier) as i64;
        let projected_api = (baseline.api_call_volume as f64 * req.transaction_volume_multiplier) as i64;

        let drivers = BusinessDrivers {
            active_merchants: projected_merchants,
            active_agents: projected_agents,
            daily_transactions: projected_tx,
            peak_tps: projected_tps,
            api_call_volume: projected_api,
        };

        let resources = rcu.project(&drivers);
        // Storage grows over the timeframe
        let storage_growth = baseline.storage_used_gb
            + (baseline.storage_growth_gb * req.timeframe_months as f64 * 30.0
                * req.transaction_volume_multiplier);

        let projected_resources = ProjectedResources {
            storage_gb: storage_growth,
            ..resources
        };

        let cost = pricing.compute_cost(&projected_resources);

        // Baseline cost for delta
        let baseline_drivers = BusinessDrivers {
            active_merchants: baseline.active_merchants,
            active_agents: baseline.active_agents,
            daily_transactions: baseline.daily_transactions,
            peak_tps: baseline.peak_tps,
            api_call_volume: baseline.api_call_volume,
        };
        let baseline_resources = rcu.project(&baseline_drivers);
        let baseline_cost = pricing.compute_cost(&baseline_resources);
        let cost_delta = cost.total_cost_usd - baseline_cost.total_cost_usd;

        let scenario = CapacityScenario {
            id: Uuid::new_v4(),
            name: req.name,
            description: req.description,
            transaction_volume_multiplier: req.transaction_volume_multiplier,
            timeframe_months: req.timeframe_months,
            new_merchant_chains: req.new_merchant_chains,
            new_agent_count: req.new_agent_count,
            projected_peak_tps: Some(projected_resources.peak_tps),
            projected_storage_gb: Some(projected_resources.storage_gb),
            projected_memory_gb: Some(projected_resources.memory_gb),
            projected_cpu_cores: Some(projected_resources.cpu_cores),
            projected_db_connections: Some(projected_resources.db_connections),
            projected_monthly_cost_usd: Some(cost.total_cost_usd),
            cost_delta_vs_baseline_usd: Some(cost_delta),
            cloud_provider: provider.to_string(),
            resource_breakdown: json!({
                "peak_tps": projected_resources.peak_tps,
                "cpu_cores": projected_resources.cpu_cores,
                "memory_gb": projected_resources.memory_gb,
                "storage_gb": projected_resources.storage_gb,
                "db_connections": projected_resources.db_connections,
            }),
            cost_breakdown: json!({
                "cpu_cost_usd": cost.cpu_cost_usd,
                "memory_cost_usd": cost.memory_cost_usd,
                "storage_cost_usd": cost.storage_cost_usd,
                "db_cost_usd": cost.db_cost_usd,
                "total_cost_usd": cost.total_cost_usd,
                "baseline_cost_usd": baseline_cost.total_cost_usd,
                "delta_usd": cost_delta,
            }),
            created_by: created_by.to_string(),
            created_at: Utc::now(),
        };

        let saved = self
            .repo
            .insert_scenario(&scenario)
            .await
            .map_err(|e| format!("Scenario save failed: {e}"))?;

        info!(scenario_id = %saved.id, "What-if scenario computed");
        Ok(saved)
    }

    // ── Cost projections ──────────────────────────────────────────────────────

    pub async fn project_costs(
        &self,
        months: i32,
        provider: &str,
    ) -> Result<Vec<CostProjection>, String> {
        let today = Utc::now().date_naive();
        let rcu = self
            .repo
            .latest_rcu()
            .await
            .map_err(|e| format!("RCU fetch: {e}"))?
            .ok_or("No RCU model")?;

        let pricing = CloudPricingConfig::from_name(provider);

        let forecasts_90 = self
            .repo
            .forecasts_for_horizon(ForecastHorizon::Rolling90d, today)
            .await
            .map_err(|e| format!("Forecast fetch: {e}"))?;

        let forecasts_12m = self
            .repo
            .forecasts_for_horizon(ForecastHorizon::Annual12m, today)
            .await
            .map_err(|e| format!("Forecast fetch: {e}"))?;

        let all_forecasts: Vec<&CapacityForecast> =
            forecasts_90.iter().chain(forecasts_12m.iter()).collect();

        let mut projections = Vec::new();
        let mut prev_cost: Option<f64> = None;

        for m in 1..=months {
            let proj_month_date = {
                let d = today + Duration::days(m as i64 * 30);
                NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()
            };

            // Pick forecast values for this month
            let tps = forecast_value(&all_forecasts, proj_month_date, ForecastMetric::Tps)
                .unwrap_or(0.0);
            let storage = forecast_value(&all_forecasts, proj_month_date, ForecastMetric::StorageGb)
                .unwrap_or(0.0);
            let mem = forecast_value(&all_forecasts, proj_month_date, ForecastMetric::MemoryGb)
                .unwrap_or(0.0);
            let db = forecast_value(&all_forecasts, proj_month_date, ForecastMetric::DbConnections)
                .unwrap_or(0.0);
            let cpu = forecast_value(&all_forecasts, proj_month_date, ForecastMetric::CpuCores)
                .unwrap_or(0.0);

            let resources = ProjectedResources {
                peak_tps: tps,
                cpu_cores: cpu * rcu.overhead_multiplier,
                memory_gb: mem * rcu.overhead_multiplier,
                storage_gb: storage,
                db_connections: (db * rcu.overhead_multiplier).ceil() as i32,
            };

            let cost = pricing.compute_cost(&resources);
            let delta_pct = prev_cost.map(|p| {
                if p > 0.0 { (cost.total_cost_usd - p) / p * 100.0 } else { 0.0 }
            });

            let proj = CostProjection {
                id: Uuid::new_v4(),
                projection_month: proj_month_date,
                cloud_provider: provider.to_string(),
                cpu_cores: resources.cpu_cores,
                memory_gb: resources.memory_gb,
                storage_gb: resources.storage_gb,
                db_connections: resources.db_connections,
                cpu_cost_usd: cost.cpu_cost_usd,
                memory_cost_usd: cost.memory_cost_usd,
                storage_cost_usd: cost.storage_cost_usd,
                db_cost_usd: cost.db_cost_usd,
                total_cost_usd: cost.total_cost_usd,
                prev_month_cost_usd: prev_cost,
                cost_delta_pct: delta_pct,
                source: "forecast".to_string(),
                scenario_id: None,
                created_at: Utc::now(),
            };

            let _ = self.repo.upsert_cost_projection(&proj).await;
            prev_cost = Some(cost.total_cost_usd);
            projections.push(proj);
        }

        Ok(projections)
    }

    // ── Capacity alerts ───────────────────────────────────────────────────────

    /// Evaluate all forecasts and fire alerts ≥60 days before projected breach.
    pub async fn evaluate_alerts(&self) -> Result<usize, String> {
        let today = Utc::now().date_naive();
        let mut fired = 0usize;

        let forecasts = {
            let mut v = self
                .repo
                .forecasts_for_horizon(ForecastHorizon::Rolling90d, today)
                .await
                .map_err(|e| format!("Forecast fetch: {e}"))?;
            v.extend(
                self.repo
                    .forecasts_for_horizon(ForecastHorizon::Annual12m, today)
                    .await
                    .map_err(|e| format!("Forecast fetch: {e}"))?,
            );
            v
        };

        let checks: &[(ForecastMetric, CapacityAlertResource, f64, &str)] = &[
            (ForecastMetric::StorageGb,    CapacityAlertResource::Storage,       STORAGE_CEILING_GB, "GB"),
            (ForecastMetric::Tps,          CapacityAlertResource::Tps,           TPS_CEILING,        "TPS"),
            (ForecastMetric::DbConnections,CapacityAlertResource::DbConnections, DB_CONN_CEILING,    "connections"),
            (ForecastMetric::MemoryGb,     CapacityAlertResource::Memory,        MEMORY_CEILING_GB,  "GB"),
        ];

        for (metric, resource, ceiling, unit) in checks {
            // Find the earliest forecast date where predicted_value >= ceiling
            if let Some(breach) = forecasts
                .iter()
                .filter(|f| f.metric == *metric && f.predicted_value >= *ceiling)
                .min_by_key(|f| f.target_date)
            {
                let days_until = (breach.target_date - today).num_days() as i32;
                if days_until <= ALERT_LEAD_DAYS * 2 {
                    let severity = if days_until <= ALERT_LEAD_DAYS {
                        CapacityAlertSeverity::Critical
                    } else {
                        CapacityAlertSeverity::Warning
                    };

                    let current = forecasts
                        .iter()
                        .filter(|f| f.metric == *metric)
                        .min_by_key(|f| f.target_date)
                        .map(|f| f.predicted_value)
                        .unwrap_or(0.0);

                    let alert = CapacityAlert {
                        id: Uuid::new_v4(),
                        resource: *resource,
                        severity,
                        projected_breach_date: breach.target_date,
                        days_until_breach: days_until,
                        current_value: current,
                        threshold_value: *ceiling,
                        projected_value: breach.predicted_value,
                        message: format!(
                            "{} projected to reach {:.1} {} (ceiling: {:.1}) in {} days on {}",
                            metric.label(), breach.predicted_value, unit,
                            ceiling, days_until, breach.target_date
                        ),
                        notified_at: None,
                        acknowledged_by: None,
                        acknowledged_at: None,
                        resolved_at: None,
                        review_task_id: None,
                        created_at: Utc::now(),
                    };

                    let _ = self.repo.insert_alert(&alert).await;
                    fired += 1;
                }
            }
        }

        if fired > 0 {
            tracing::warn!(fired, "Capacity alerts fired");
        }
        Ok(fired)
    }

    // ── Management dashboard ──────────────────────────────────────────────────

    pub async fn management_dashboard(&self) -> Result<CapacityDashboard, String> {
        let alerts = self
            .repo
            .list_alerts(Some(false))
            .await
            .map_err(|e| format!("Alert fetch: {e}"))?;

        let costs = self.project_costs(12, "aws").await.unwrap_or_default();
        let monthly_burn = costs.first().map(|c| c.total_cost_usd).unwrap_or(0.0);
        let annual = costs.iter().map(|c| c.total_cost_usd).sum::<f64>();

        let critical: Vec<AlertSummary> = alerts
            .iter()
            .filter(|a| matches!(a.severity, CapacityAlertSeverity::Critical))
            .map(|a| AlertSummary {
                resource: format!("{:?}", a.resource),
                message: a.message.clone(),
                days_until_breach: a.days_until_breach,
                severity: "critical".into(),
            })
            .collect();

        let capacity_health = if critical.is_empty() && alerts.is_empty() {
            "Healthy — all resources within safe limits"
        } else if critical.is_empty() {
            "Watch — some resources approaching limits"
        } else {
            "Action Required — critical capacity thresholds approaching"
        };

        let outlook = build_outlook(&alerts);

        Ok(CapacityDashboard {
            generated_at: Utc::now(),
            peg_status: "operational".into(),
            capacity_health: capacity_health.into(),
            outlook_90d: outlook,
            monthly_burn_rate_usd: monthly_burn,
            projected_annual_cost_usd: annual,
            active_alerts: alerts.len(),
            critical_alerts: critical,
        })
    }

    // ── Quarterly report ──────────────────────────────────────────────────────

    pub async fn generate_quarterly_report(&self) -> Result<QuarterlyReport, String> {
        let today = Utc::now().date_naive();
        let quarter = format!("{}-Q{}", today.year(), (today.month() - 1) / 3 + 1);

        let history = self
            .repo
            .recent_metrics(90)
            .await
            .map_err(|e| format!("History fetch: {e}"))?;

        let accuracy = self.compute_forecast_accuracy().await.ok();

        let growth_summary = if history.len() >= 2 {
            let first = &history[0];
            let last = &history[history.len() - 1];
            json!({
                "period_days": history.len(),
                "merchant_growth_pct": pct_change(first.active_merchants as f64, last.active_merchants as f64),
                "agent_growth_pct": pct_change(first.active_agents as f64, last.active_agents as f64),
                "transaction_growth_pct": pct_change(first.daily_transactions as f64, last.daily_transactions as f64),
                "tps_growth_pct": pct_change(first.peak_tps, last.peak_tps),
                "storage_growth_gb": last.storage_used_gb - first.storage_used_gb,
            })
        } else {
            json!({})
        };

        let costs = self.project_costs(3, "aws").await.unwrap_or_default();
        let capacity_requirements = json!({
            "next_quarter_cost_usd": costs.iter().map(|c| c.total_cost_usd).sum::<f64>(),
            "peak_tps_projected": costs.first().map(|_| "see forecasts").unwrap_or("n/a"),
        });

        let recommendations = json!([
            "Review storage provisioning if growth exceeds 15% QoQ",
            "Scale DB connection pool if agent count grows >20%",
            "Evaluate reserved instance pricing for >6-month commitments",
        ]);

        let executive_summary = format!(
            "Q{} Capacity Report: Platform grew {:.1}% in transactions over the quarter. \
             Projected infrastructure burn rate for next quarter: ${:.0}/month. \
             {} active capacity alerts. Forecast accuracy: {:.1}%.",
            (today.month() - 1) / 3 + 1,
            growth_summary["transaction_growth_pct"].as_f64().unwrap_or(0.0),
            costs.first().map(|c| c.total_cost_usd).unwrap_or(0.0),
            0usize,
            accuracy.unwrap_or(0.0),
        );

        let report = QuarterlyReport {
            id: Uuid::new_v4(),
            quarter: quarter.clone(),
            report_date: today,
            growth_summary,
            capacity_requirements,
            recommendations,
            prev_quarter_accuracy_pct: accuracy,
            executive_summary: Some(executive_summary),
            full_report: json!({ "generated_at": today.to_string() }),
            generated_by: "capacity_worker".into(),
            created_at: Utc::now(),
        };

        self.repo
            .upsert_quarterly_report(&report)
            .await
            .map_err(|e| format!("Report save failed: {e}"))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn forecast_value(
    forecasts: &[&CapacityForecast],
    target: NaiveDate,
    metric: ForecastMetric,
) -> Option<f64> {
    forecasts
        .iter()
        .filter(|f| f.metric == metric && f.target_date <= target)
        .max_by_key(|f| f.target_date)
        .map(|f| f.predicted_value)
}

fn build_outlook(alerts: &[CapacityAlert]) -> Vec<ResourceOutlook> {
    let resources = [
        ("Storage", CapacityAlertResource::Storage),
        ("TPS", CapacityAlertResource::Tps),
        ("Memory", CapacityAlertResource::Memory),
        ("DB Connections", CapacityAlertResource::DbConnections),
    ];

    resources
        .iter()
        .map(|(name, res)| {
            let alert = alerts.iter().find(|a| a.resource == *res);
            match alert {
                None => ResourceOutlook {
                    resource: name.to_string(),
                    status: "healthy".into(),
                    plain_language: format!("{name} is within safe limits for the next 90 days"),
                    days_to_threshold: None,
                },
                Some(a) if matches!(a.severity, CapacityAlertSeverity::Critical) => ResourceOutlook {
                    resource: name.to_string(),
                    status: "action_required".into(),
                    plain_language: format!(
                        "{name} will reach capacity in {} days — immediate provisioning required",
                        a.days_until_breach
                    ),
                    days_to_threshold: Some(a.days_until_breach),
                },
                Some(a) => ResourceOutlook {
                    resource: name.to_string(),
                    status: "watch".into(),
                    plain_language: format!(
                        "{name} is trending toward capacity in {} days — plan provisioning",
                        a.days_until_breach
                    ),
                    days_to_threshold: Some(a.days_until_breach),
                },
            }
        })
        .collect()
}

fn pct_change(from: f64, to: f64) -> f64 {
    if from == 0.0 { 0.0 } else { (to - from) / from * 100.0 }
}
