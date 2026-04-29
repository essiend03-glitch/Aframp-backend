/// Capacity Planning Repository — all DB queries.
use super::types::*;
use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

pub struct CapacityRepository {
    db: PgPool,
}

impl CapacityRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &PgPool {
        &self.db
    }

    // ── Business Metrics ──────────────────────────────────────────────────────

    pub async fn upsert_business_metrics(
        &self,
        r: &IngestMetricsRequest,
    ) -> Result<BusinessMetricRow, sqlx::Error> {
        sqlx::query_as!(
            BusinessMetricRow,
            r#"
            INSERT INTO capacity_business_metrics
                (metric_date, active_merchants, active_agents, daily_transactions,
                 peak_tps, avg_transaction_size_kb, api_call_volume, db_connections_peak,
                 storage_used_gb, storage_growth_gb, avg_cpu_pct, avg_memory_gb,
                 corridor_breakdown)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            ON CONFLICT (metric_date) DO UPDATE SET
                active_merchants        = EXCLUDED.active_merchants,
                active_agents           = EXCLUDED.active_agents,
                daily_transactions      = EXCLUDED.daily_transactions,
                peak_tps                = EXCLUDED.peak_tps,
                avg_transaction_size_kb = EXCLUDED.avg_transaction_size_kb,
                api_call_volume         = EXCLUDED.api_call_volume,
                db_connections_peak     = EXCLUDED.db_connections_peak,
                storage_used_gb         = EXCLUDED.storage_used_gb,
                storage_growth_gb       = EXCLUDED.storage_growth_gb,
                avg_cpu_pct             = EXCLUDED.avg_cpu_pct,
                avg_memory_gb           = EXCLUDED.avg_memory_gb,
                corridor_breakdown      = EXCLUDED.corridor_breakdown
            RETURNING
                id, metric_date, active_merchants, active_agents, daily_transactions,
                peak_tps::float8, avg_transaction_size_kb::float8, api_call_volume,
                db_connections_peak, storage_used_gb::float8, storage_growth_gb::float8,
                avg_cpu_pct::float8, avg_memory_gb::float8, corridor_breakdown, created_at
            "#,
            r.metric_date,
            r.active_merchants,
            r.active_agents,
            r.daily_transactions,
            r.peak_tps as f64,
            r.avg_transaction_size_kb as f64,
            r.api_call_volume,
            r.db_connections_peak,
            r.storage_used_gb as f64,
            r.storage_growth_gb as f64,
            r.avg_cpu_pct as f64,
            r.avg_memory_gb as f64,
            r.corridor_breakdown.clone().unwrap_or(serde_json::json!({})),
        )
        .fetch_one(&self.db)
        .await
    }

    /// Fetch the last N days of business metrics for forecasting.
    pub async fn recent_metrics(
        &self,
        days: i64,
    ) -> Result<Vec<BusinessMetricRow>, sqlx::Error> {
        sqlx::query_as!(
            BusinessMetricRow,
            r#"
            SELECT id, metric_date, active_merchants, active_agents, daily_transactions,
                   peak_tps::float8, avg_transaction_size_kb::float8, api_call_volume,
                   db_connections_peak, storage_used_gb::float8, storage_growth_gb::float8,
                   avg_cpu_pct::float8, avg_memory_gb::float8, corridor_breakdown, created_at
            FROM capacity_business_metrics
            WHERE metric_date >= CURRENT_DATE - ($1 || ' days')::INTERVAL
            ORDER BY metric_date ASC
            "#,
            days.to_string(),
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn latest_metric(&self) -> Result<Option<BusinessMetricRow>, sqlx::Error> {
        sqlx::query_as!(
            BusinessMetricRow,
            r#"
            SELECT id, metric_date, active_merchants, active_agents, daily_transactions,
                   peak_tps::float8, avg_transaction_size_kb::float8, api_call_volume,
                   db_connections_peak, storage_used_gb::float8, storage_growth_gb::float8,
                   avg_cpu_pct::float8, avg_memory_gb::float8, corridor_breakdown, created_at
            FROM capacity_business_metrics
            ORDER BY metric_date DESC LIMIT 1
            "#
        )
        .fetch_optional(&self.db)
        .await
    }

    // ── Resource Consumption Unit model ──────────────────────────────────────

    pub async fn latest_rcu(&self) -> Result<Option<ResourceConsumptionUnit>, sqlx::Error> {
        sqlx::query_as!(
            ResourceConsumptionUnit,
            r#"
            SELECT id, model_month, cpu_cores_per_1k_tps::float8,
                   memory_gb_per_1k_tps::float8, disk_iops_per_1k_tps::float8,
                   storage_gb_per_1k_tx::float8, db_connections_per_agent::float8,
                   db_connections_per_merchant::float8, memory_mb_per_api_call::float8,
                   overhead_multiplier::float8, forecast_accuracy_pct::float8 AS "forecast_accuracy_pct?",
                   computed_by, notes, created_at
            FROM capacity_resource_units
            ORDER BY model_month DESC LIMIT 1
            "#
        )
        .fetch_optional(&self.db)
        .await
    }

    pub async fn upsert_rcu(
        &self,
        month: NaiveDate,
        cpu: f64, mem: f64, iops: f64, storage: f64,
        db_agent: f64, db_merchant: f64, mem_api: f64,
        overhead: f64, accuracy: Option<f64>,
    ) -> Result<ResourceConsumptionUnit, sqlx::Error> {
        sqlx::query_as!(
            ResourceConsumptionUnit,
            r#"
            INSERT INTO capacity_resource_units
                (model_month, cpu_cores_per_1k_tps, memory_gb_per_1k_tps,
                 disk_iops_per_1k_tps, storage_gb_per_1k_tx,
                 db_connections_per_agent, db_connections_per_merchant,
                 memory_mb_per_api_call, overhead_multiplier, forecast_accuracy_pct)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            ON CONFLICT (model_month) DO UPDATE SET
                cpu_cores_per_1k_tps        = EXCLUDED.cpu_cores_per_1k_tps,
                memory_gb_per_1k_tps        = EXCLUDED.memory_gb_per_1k_tps,
                disk_iops_per_1k_tps        = EXCLUDED.disk_iops_per_1k_tps,
                storage_gb_per_1k_tx        = EXCLUDED.storage_gb_per_1k_tx,
                db_connections_per_agent    = EXCLUDED.db_connections_per_agent,
                db_connections_per_merchant = EXCLUDED.db_connections_per_merchant,
                memory_mb_per_api_call      = EXCLUDED.memory_mb_per_api_call,
                overhead_multiplier         = EXCLUDED.overhead_multiplier,
                forecast_accuracy_pct       = EXCLUDED.forecast_accuracy_pct
            RETURNING id, model_month, cpu_cores_per_1k_tps::float8,
                      memory_gb_per_1k_tps::float8, disk_iops_per_1k_tps::float8,
                      storage_gb_per_1k_tx::float8, db_connections_per_agent::float8,
                      db_connections_per_merchant::float8, memory_mb_per_api_call::float8,
                      overhead_multiplier::float8,
                      forecast_accuracy_pct::float8 AS "forecast_accuracy_pct?",
                      computed_by, notes, created_at
            "#,
            month, cpu, mem, iops, storage, db_agent, db_merchant, mem_api, overhead, accuracy,
        )
        .fetch_one(&self.db)
        .await
    }

    // ── Forecasts ─────────────────────────────────────────────────────────────

    pub async fn insert_forecast(
        &self,
        forecast_date: NaiveDate,
        target_date: NaiveDate,
        horizon: ForecastHorizon,
        metric: ForecastMetric,
        predicted: f64,
        lower: f64,
        upper: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO capacity_forecasts
                (forecast_date, target_date, horizon, metric,
                 predicted_value, lower_bound, upper_bound)
            VALUES ($1,$2,$3::forecast_horizon,$4::forecast_metric,$5,$6,$7)
            ON CONFLICT (forecast_date, target_date, horizon, metric) DO UPDATE SET
                predicted_value = EXCLUDED.predicted_value,
                lower_bound     = EXCLUDED.lower_bound,
                upper_bound     = EXCLUDED.upper_bound
            "#,
            forecast_date, target_date,
            horizon as ForecastHorizon,
            metric as ForecastMetric,
            predicted, lower, upper,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn forecasts_for_horizon(
        &self,
        horizon: ForecastHorizon,
        forecast_date: NaiveDate,
    ) -> Result<Vec<CapacityForecast>, sqlx::Error> {
        sqlx::query_as!(
            CapacityForecast,
            r#"
            SELECT id, forecast_date, target_date,
                   horizon AS "horizon: ForecastHorizon",
                   metric  AS "metric: ForecastMetric",
                   predicted_value::float8, lower_bound::float8, upper_bound::float8,
                   actual_value::float8 AS "actual_value?",
                   ape_pct::float8 AS "ape_pct?",
                   model_version, created_at
            FROM capacity_forecasts
            WHERE horizon = $1::forecast_horizon
              AND forecast_date = $2
            ORDER BY target_date ASC, metric ASC
            "#,
            horizon as ForecastHorizon,
            forecast_date,
        )
        .fetch_all(&self.db)
        .await
    }

    /// Backfill actual values for past forecast targets (for accuracy tracking).
    pub async fn backfill_actuals(&self, target_date: NaiveDate) -> Result<(), sqlx::Error> {
        // Pull actual metric values from business_metrics for that date
        sqlx::query!(
            r#"
            UPDATE capacity_forecasts cf
            SET actual_value = CASE cf.metric
                WHEN 'tps'              THEN bm.peak_tps
                WHEN 'storage_gb'       THEN bm.storage_used_gb
                WHEN 'db_connections'   THEN bm.db_connections_peak::numeric
                WHEN 'memory_gb'        THEN bm.avg_memory_gb
                WHEN 'cpu_cores'        THEN bm.avg_cpu_pct / 100.0 * 32  -- normalise to cores
                WHEN 'active_merchants' THEN bm.active_merchants::numeric
                WHEN 'active_agents'    THEN bm.active_agents::numeric
                ELSE NULL
            END,
            ape_pct = CASE
                WHEN cf.predicted_value > 0 THEN
                    ABS(CASE cf.metric
                        WHEN 'tps'              THEN bm.peak_tps
                        WHEN 'storage_gb'       THEN bm.storage_used_gb
                        WHEN 'db_connections'   THEN bm.db_connections_peak::numeric
                        WHEN 'memory_gb'        THEN bm.avg_memory_gb
                        WHEN 'active_merchants' THEN bm.active_merchants::numeric
                        WHEN 'active_agents'    THEN bm.active_agents::numeric
                        ELSE cf.predicted_value
                    END - cf.predicted_value) / cf.predicted_value * 100
                ELSE NULL
            END
            FROM capacity_business_metrics bm
            WHERE cf.target_date = $1
              AND bm.metric_date = $1
              AND cf.actual_value IS NULL
            "#,
            target_date,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    // ── Scenarios ─────────────────────────────────────────────────────────────

    pub async fn insert_scenario(
        &self,
        s: &CapacityScenario,
    ) -> Result<CapacityScenario, sqlx::Error> {
        sqlx::query_as!(
            CapacityScenario,
            r#"
            INSERT INTO capacity_scenarios
                (id, name, description, transaction_volume_multiplier, timeframe_months,
                 new_merchant_chains, new_agent_count, projected_peak_tps, projected_storage_gb,
                 projected_memory_gb, projected_cpu_cores, projected_db_connections,
                 projected_monthly_cost_usd, cost_delta_vs_baseline_usd, cloud_provider,
                 resource_breakdown, cost_breakdown, created_by)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
            RETURNING id, name, description,
                      transaction_volume_multiplier::float8, timeframe_months,
                      new_merchant_chains, new_agent_count,
                      projected_peak_tps::float8 AS "projected_peak_tps?",
                      projected_storage_gb::float8 AS "projected_storage_gb?",
                      projected_memory_gb::float8 AS "projected_memory_gb?",
                      projected_cpu_cores::float8 AS "projected_cpu_cores?",
                      projected_db_connections AS "projected_db_connections?",
                      projected_monthly_cost_usd::float8 AS "projected_monthly_cost_usd?",
                      cost_delta_vs_baseline_usd::float8 AS "cost_delta_vs_baseline_usd?",
                      cloud_provider, resource_breakdown, cost_breakdown, created_by, created_at
            "#,
            s.id, s.name, s.description,
            s.transaction_volume_multiplier, s.timeframe_months,
            s.new_merchant_chains, s.new_agent_count,
            s.projected_peak_tps, s.projected_storage_gb,
            s.projected_memory_gb, s.projected_cpu_cores,
            s.projected_db_connections,
            s.projected_monthly_cost_usd, s.cost_delta_vs_baseline_usd,
            s.cloud_provider, s.resource_breakdown, s.cost_breakdown, s.created_by,
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn list_scenarios(&self, limit: i64) -> Result<Vec<CapacityScenario>, sqlx::Error> {
        sqlx::query_as!(
            CapacityScenario,
            r#"
            SELECT id, name, description,
                   transaction_volume_multiplier::float8, timeframe_months,
                   new_merchant_chains, new_agent_count,
                   projected_peak_tps::float8 AS "projected_peak_tps?",
                   projected_storage_gb::float8 AS "projected_storage_gb?",
                   projected_memory_gb::float8 AS "projected_memory_gb?",
                   projected_cpu_cores::float8 AS "projected_cpu_cores?",
                   projected_db_connections AS "projected_db_connections?",
                   projected_monthly_cost_usd::float8 AS "projected_monthly_cost_usd?",
                   cost_delta_vs_baseline_usd::float8 AS "cost_delta_vs_baseline_usd?",
                   cloud_provider, resource_breakdown, cost_breakdown, created_by, created_at
            FROM capacity_scenarios
            ORDER BY created_at DESC LIMIT $1
            "#,
            limit,
        )
        .fetch_all(&self.db)
        .await
    }

    // ── Alerts ────────────────────────────────────────────────────────────────

    pub async fn insert_alert(&self, a: &CapacityAlert) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO capacity_alerts
                (id, resource, severity, projected_breach_date, days_until_breach,
                 current_value, threshold_value, projected_value, message)
            VALUES ($1,$2::capacity_alert_resource,$3::capacity_alert_severity,
                    $4,$5,$6,$7,$8,$9)
            ON CONFLICT DO NOTHING
            "#,
            a.id,
            a.resource as CapacityAlertResource,
            a.severity as CapacityAlertSeverity,
            a.projected_breach_date,
            a.days_until_breach,
            a.current_value, a.threshold_value, a.projected_value,
            a.message,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn list_alerts(
        &self,
        resolved: Option<bool>,
    ) -> Result<Vec<CapacityAlert>, sqlx::Error> {
        sqlx::query_as!(
            CapacityAlert,
            r#"
            SELECT id,
                   resource  AS "resource: CapacityAlertResource",
                   severity  AS "severity: CapacityAlertSeverity",
                   projected_breach_date, days_until_breach,
                   current_value::float8, threshold_value::float8, projected_value::float8,
                   message, notified_at, acknowledged_by, acknowledged_at, resolved_at,
                   review_task_id, created_at
            FROM capacity_alerts
            WHERE ($1::boolean IS NULL OR (resolved_at IS NULL) = NOT $1)
            ORDER BY days_until_breach ASC, created_at DESC
            "#,
            resolved,
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn acknowledge_alert(
        &self,
        id: Uuid,
        by: &str,
        task_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE capacity_alerts
               SET acknowledged_by = $2, acknowledged_at = NOW(),
                   review_task_id = COALESCE($3, review_task_id)
             WHERE id = $1
            "#,
            id, by, task_id,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    // ── Cost projections ──────────────────────────────────────────────────────

    pub async fn upsert_cost_projection(
        &self,
        p: &CostProjection,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO capacity_cost_projections
                (projection_month, cloud_provider, cpu_cores, memory_gb, storage_gb,
                 db_connections, cpu_cost_usd, memory_cost_usd, storage_cost_usd,
                 db_cost_usd, total_cost_usd, prev_month_cost_usd, cost_delta_pct,
                 source, scenario_id)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
            ON CONFLICT (projection_month, cloud_provider, source,
                COALESCE(scenario_id, '00000000-0000-0000-0000-000000000000'::uuid))
            DO UPDATE SET
                cpu_cores    = EXCLUDED.cpu_cores,
                memory_gb    = EXCLUDED.memory_gb,
                storage_gb   = EXCLUDED.storage_gb,
                total_cost_usd = EXCLUDED.total_cost_usd
            "#,
            p.projection_month, p.cloud_provider,
            p.cpu_cores, p.memory_gb, p.storage_gb, p.db_connections,
            p.cpu_cost_usd, p.memory_cost_usd, p.storage_cost_usd, p.db_cost_usd,
            p.total_cost_usd, p.prev_month_cost_usd, p.cost_delta_pct,
            p.source, p.scenario_id,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn cost_projections(
        &self,
        months: i32,
        provider: &str,
    ) -> Result<Vec<CostProjection>, sqlx::Error> {
        sqlx::query_as!(
            CostProjection,
            r#"
            SELECT id, projection_month, cloud_provider,
                   cpu_cores::float8, memory_gb::float8, storage_gb::float8,
                   db_connections, cpu_cost_usd::float8, memory_cost_usd::float8,
                   storage_cost_usd::float8, db_cost_usd::float8, total_cost_usd::float8,
                   prev_month_cost_usd::float8 AS "prev_month_cost_usd?",
                   cost_delta_pct::float8 AS "cost_delta_pct?",
                   source, scenario_id, created_at
            FROM capacity_cost_projections
            WHERE cloud_provider = $1
              AND projection_month >= CURRENT_DATE
              AND source = 'forecast'
            ORDER BY projection_month ASC
            LIMIT $2
            "#,
            provider, months as i64,
        )
        .fetch_all(&self.db)
        .await
    }

    // ── Quarterly reports ─────────────────────────────────────────────────────

    pub async fn upsert_quarterly_report(
        &self,
        r: &QuarterlyReport,
    ) -> Result<QuarterlyReport, sqlx::Error> {
        sqlx::query_as!(
            QuarterlyReport,
            r#"
            INSERT INTO capacity_quarterly_reports
                (id, quarter, report_date, growth_summary, capacity_requirements,
                 recommendations, prev_quarter_accuracy_pct, executive_summary, full_report)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            ON CONFLICT (quarter) DO UPDATE SET
                growth_summary          = EXCLUDED.growth_summary,
                capacity_requirements   = EXCLUDED.capacity_requirements,
                recommendations         = EXCLUDED.recommendations,
                executive_summary       = EXCLUDED.executive_summary,
                full_report             = EXCLUDED.full_report
            RETURNING id, quarter, report_date, growth_summary, capacity_requirements,
                      recommendations,
                      prev_quarter_accuracy_pct::float8 AS "prev_quarter_accuracy_pct?",
                      executive_summary, full_report, generated_by, created_at
            "#,
            r.id, r.quarter, r.report_date,
            r.growth_summary, r.capacity_requirements, r.recommendations,
            r.prev_quarter_accuracy_pct, r.executive_summary, r.full_report,
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn latest_quarterly_report(
        &self,
    ) -> Result<Option<QuarterlyReport>, sqlx::Error> {
        sqlx::query_as!(
            QuarterlyReport,
            r#"
            SELECT id, quarter, report_date, growth_summary, capacity_requirements,
                   recommendations,
                   prev_quarter_accuracy_pct::float8 AS "prev_quarter_accuracy_pct?",
                   executive_summary, full_report, generated_by, created_at
            FROM capacity_quarterly_reports
            ORDER BY quarter DESC LIMIT 1
            "#
        )
        .fetch_optional(&self.db)
        .await
    }
}
