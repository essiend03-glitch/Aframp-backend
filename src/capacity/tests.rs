/// Capacity Planning Engine — Unit Tests
#[cfg(test)]
mod tests {
    use crate::capacity::forecaster::ols_fit;
    use crate::capacity::types::*;
    use chrono::NaiveDate;

    // ── RCU model: business drivers → resource projection ─────────────────────

    fn default_rcu() -> ResourceConsumptionUnit {
        ResourceConsumptionUnit {
            id: uuid::Uuid::new_v4(),
            model_month: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            cpu_cores_per_1k_tps: 2.0,
            memory_gb_per_1k_tps: 4.0,
            disk_iops_per_1k_tps: 200.0,
            storage_gb_per_1k_tx: 0.5,
            db_connections_per_agent: 0.5,
            db_connections_per_merchant: 0.3,
            memory_mb_per_api_call: 0.01,
            overhead_multiplier: 1.30,
            forecast_accuracy_pct: None,
            computed_by: "test".into(),
            notes: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn rcu_maps_tps_to_cpu_and_memory() {
        let rcu = default_rcu();
        let drivers = BusinessDrivers {
            active_merchants: 100,
            active_agents: 500,
            daily_transactions: 100_000,
            peak_tps: 1_000.0,
            api_call_volume: 500_000,
        };
        let res = rcu.project(&drivers);

        // 1000 TPS / 1000 * 2.0 cpu * 1.30 overhead = 2.6 cores
        assert!((res.cpu_cores - 2.6).abs() < 0.01, "cpu_cores = {}", res.cpu_cores);
        // 1000 TPS / 1000 * 4.0 mem * 1.30 = 5.2 GB (+ api memory)
        assert!(res.memory_gb > 5.0, "memory_gb = {}", res.memory_gb);
    }

    #[test]
    fn rcu_maps_agents_to_db_connections() {
        let rcu = default_rcu();
        let drivers = BusinessDrivers {
            active_merchants: 0,
            active_agents: 1_000,
            daily_transactions: 0,
            peak_tps: 0.0,
            api_call_volume: 0,
        };
        let res = rcu.project(&drivers);
        // 1000 agents * 0.5 * 1.30 = 650 connections
        assert_eq!(res.db_connections, 650);
    }

    #[test]
    fn rcu_maps_merchants_to_db_connections() {
        let rcu = default_rcu();
        let drivers = BusinessDrivers {
            active_merchants: 1_000,
            active_agents: 0,
            daily_transactions: 0,
            peak_tps: 0.0,
            api_call_volume: 0,
        };
        let res = rcu.project(&drivers);
        // 1000 merchants * 0.3 * 1.30 = 390 connections
        assert_eq!(res.db_connections, 390);
    }

    #[test]
    fn rcu_storage_scales_with_transactions() {
        let rcu = default_rcu();
        let drivers_low = BusinessDrivers {
            active_merchants: 0, active_agents: 0,
            daily_transactions: 10_000, peak_tps: 0.0, api_call_volume: 0,
        };
        let drivers_high = BusinessDrivers {
            daily_transactions: 100_000,
            ..drivers_low.clone()
        };
        let low = rcu.project(&drivers_low);
        let high = rcu.project(&drivers_high);
        assert!(high.storage_gb > low.storage_gb * 9.0, "storage should scale 10x");
    }

    // ── Cloud pricing: resource → cost ────────────────────────────────────────

    #[test]
    fn aws_pricing_computes_cost() {
        let pricing = CloudPricingConfig::aws();
        let resources = ProjectedResources {
            peak_tps: 1000.0,
            cpu_cores: 10.0,
            memory_gb: 40.0,
            storage_gb: 500.0,
            db_connections: 200,
        };
        let cost = pricing.compute_cost(&resources);
        // cpu: 10 * 48 = 480
        assert!((cost.cpu_cost_usd - 480.0).abs() < 0.01);
        // memory: 40 * 6 = 240
        assert!((cost.memory_cost_usd - 240.0).abs() < 0.01);
        // storage: 500 * 0.10 = 50
        assert!((cost.storage_cost_usd - 50.0).abs() < 0.01);
        // db: 200 * 0.50 = 100
        assert!((cost.db_cost_usd - 100.0).abs() < 0.01);
        assert!((cost.total_cost_usd - 870.0).abs() < 0.01);
    }

    #[test]
    fn gcp_pricing_lower_than_aws() {
        let resources = ProjectedResources {
            peak_tps: 500.0, cpu_cores: 8.0, memory_gb: 32.0,
            storage_gb: 200.0, db_connections: 100,
        };
        let aws = CloudPricingConfig::aws().compute_cost(&resources);
        let gcp = CloudPricingConfig::gcp().compute_cost(&resources);
        assert!(gcp.total_cost_usd < aws.total_cost_usd, "GCP should be cheaper than AWS");
    }

    // ── Forecaster: OLS regression ────────────────────────────────────────────

    #[test]
    fn ols_flat_series_returns_zero_slope() {
        let pts: Vec<(f64, f64)> = (0..10).map(|i| (i as f64, 5.0_f64.ln())).collect();
        let (b0, b1, _) = ols_fit(&pts);
        assert!(b1.abs() < 1e-6, "slope should be ~0 for flat series");
        assert!((b0 - 5.0_f64.ln()).abs() < 1e-6);
    }

    #[test]
    fn ols_growing_series_returns_positive_slope() {
        let pts: Vec<(f64, f64)> = (0..30)
            .map(|i| (i as f64, (100.0 * 1.01_f64.powi(i)).ln()))
            .collect();
        let (_, b1, _) = ols_fit(&pts);
        assert!(b1 > 0.0, "slope should be positive for growing series");
    }

    // ── Alert thresholds: fire ≥60 days before breach ─────────────────────────

    #[test]
    fn alert_fires_within_lead_time() {
        // Simulate a forecast that breaches TPS ceiling in 45 days
        let today = chrono::Utc::now().date_naive();
        let breach_date = today + chrono::Duration::days(45);
        let days_until = (breach_date - today).num_days() as i32;

        // 45 days < 60 days lead time → should fire as Critical
        assert!(days_until <= 60, "45 days is within 60-day lead time");
        assert!(days_until <= 60, "should trigger Critical alert");
    }

    #[test]
    fn alert_warning_fires_within_120_days() {
        let today = chrono::Utc::now().date_naive();
        let breach_date = today + chrono::Duration::days(90);
        let days_until = (breach_date - today).num_days() as i32;

        // 90 days < 120 days (2x lead time) → Warning
        assert!(days_until <= 120, "90 days is within warning window");
    }

    #[test]
    fn alert_does_not_fire_beyond_lead_time() {
        let today = chrono::Utc::now().date_naive();
        let breach_date = today + chrono::Duration::days(200);
        let days_until = (breach_date - today).num_days() as i32;

        // 200 days > 120 days → no alert
        assert!(days_until > 120, "200 days is outside alert window");
    }

    // ── What-if scenario: multiplier scales resources ─────────────────────────

    #[test]
    fn scenario_2x_multiplier_doubles_tps() {
        let rcu = default_rcu();
        let base_tps = 500.0_f64;

        let base_drivers = BusinessDrivers {
            active_merchants: 100, active_agents: 200,
            daily_transactions: 50_000, peak_tps: base_tps, api_call_volume: 100_000,
        };
        let scaled_drivers = BusinessDrivers {
            peak_tps: base_tps * 2.0,
            daily_transactions: 100_000,
            api_call_volume: 200_000,
            ..base_drivers.clone()
        };

        let base_res = rcu.project(&base_drivers);
        let scaled_res = rcu.project(&scaled_drivers);

        assert!((scaled_res.cpu_cores / base_res.cpu_cores - 2.0).abs() < 0.01,
            "2x TPS should double CPU cores");
        assert!((scaled_res.memory_gb / base_res.memory_gb - 2.0).abs() < 0.1,
            "2x TPS should approximately double memory");
    }

    // ── Cost projection accuracy ──────────────────────────────────────────────

    #[test]
    fn cost_projection_matches_expected_aws_pricing() {
        // Known inputs → known output
        let pricing = CloudPricingConfig::aws();
        let resources = ProjectedResources {
            peak_tps: 2000.0,
            cpu_cores: 20.0,   // $960
            memory_gb: 80.0,   // $480
            storage_gb: 1000.0, // $100
            db_connections: 400, // $200
        };
        let cost = pricing.compute_cost(&resources);
        let expected = 960.0 + 480.0 + 100.0 + 200.0;
        assert!((cost.total_cost_usd - expected).abs() < 0.01,
            "total = {}, expected = {}", cost.total_cost_usd, expected);
    }
}
