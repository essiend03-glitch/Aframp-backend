//! Unit tests for DeFi analytics computation functions (Issue #348).
//!
//! Run with:
//!   cargo test --test defi_analytics_tests --features database

#[cfg(feature = "database")]
mod defi_analytics_unit_tests {
    use aframp_backend::defi::analytics::service::{
        compute_amm_capital_efficiency, compute_benchmark_delta, compute_il_vs_hold,
        compute_liquidation_rate, compute_protocol_efficiency, compute_risk_adjusted_return,
        compute_weighted_avg_yield_rate,
    };

    // ── Weighted average yield rate ───────────────────────────────────────────

    #[test]
    fn weighted_avg_yield_rate_basic() {
        // Two products: 10% yield on 1000, 5% yield on 500
        let products = vec![(0.10, 1000.0), (0.05, 500.0)];
        let result = compute_weighted_avg_yield_rate(&products);
        // (0.10 * 1000 + 0.05 * 500) / 1500 = 125 / 1500 ≈ 0.0833
        let expected = (0.10 * 1000.0 + 0.05 * 500.0) / 1500.0;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn weighted_avg_yield_rate_zero_weight() {
        let products: Vec<(f64, f64)> = vec![];
        assert_eq!(compute_weighted_avg_yield_rate(&products), 0.0);
    }

    #[test]
    fn weighted_avg_yield_rate_single_product() {
        let products = vec![(0.08, 5000.0)];
        assert!((compute_weighted_avg_yield_rate(&products) - 0.08).abs() < 1e-9);
    }

    // ── Risk-adjusted return ──────────────────────────────────────────────────

    #[test]
    fn risk_adjusted_return_no_drawdown() {
        // yield 10%, drawdown 0% → RAR = 0.10 / 1.0 = 0.10
        assert!((compute_risk_adjusted_return(0.10, 0.0) - 0.10).abs() < 1e-9);
    }

    #[test]
    fn risk_adjusted_return_with_drawdown() {
        // yield 10%, drawdown 5% → RAR = 0.10 / 1.05 ≈ 0.0952
        let expected = 0.10 / 1.05;
        assert!((compute_risk_adjusted_return(0.10, 0.05) - expected).abs() < 1e-9);
    }

    #[test]
    fn risk_adjusted_return_full_drawdown() {
        // drawdown >= 100% → 0
        assert_eq!(compute_risk_adjusted_return(0.10, 1.0), 0.0);
    }

    // ── AMM capital efficiency ────────────────────────────────────────────────

    #[test]
    fn amm_capital_efficiency_basic() {
        // volume 10_000, liquidity 5_000 → efficiency = 2.0
        assert!((compute_amm_capital_efficiency(10_000.0, 5_000.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn amm_capital_efficiency_zero_liquidity() {
        assert_eq!(compute_amm_capital_efficiency(10_000.0, 0.0), 0.0);
    }

    // ── Impermanent loss vs hold ──────────────────────────────────────────────

    #[test]
    fn il_vs_hold_positive_when_fees_exceed_il_and_hold() {
        // fee_income 200, IL 50, hold_return 100 → actual = 150, vs hold = 100 → delta = 50
        let result = compute_il_vs_hold(200.0, 50.0, 100.0);
        assert!((result - 50.0).abs() < 1e-9);
    }

    #[test]
    fn il_vs_hold_negative_when_il_dominates() {
        // fee_income 50, IL 200, hold_return 100 → actual = -150, vs hold = 100 → delta = -250
        let result = compute_il_vs_hold(50.0, 200.0, 100.0);
        assert!((result - (-250.0)).abs() < 1e-9);
    }

    // ── Liquidation rate ──────────────────────────────────────────────────────

    #[test]
    fn liquidation_rate_basic() {
        // 5 liquidations out of 100 borrowers = 5%
        assert!((compute_liquidation_rate(5, 100) - 0.05).abs() < 1e-9);
    }

    #[test]
    fn liquidation_rate_zero_borrowers() {
        assert_eq!(compute_liquidation_rate(5, 0), 0.0);
    }

    #[test]
    fn liquidation_rate_no_liquidations() {
        assert_eq!(compute_liquidation_rate(0, 100), 0.0);
    }

    // ── Protocol efficiency ───────────────────────────────────────────────────

    #[test]
    fn protocol_efficiency_basic() {
        // yield 500, capital 10_000 → efficiency = 0.05
        assert!((compute_protocol_efficiency(500.0, 10_000.0) - 0.05).abs() < 1e-9);
    }

    #[test]
    fn protocol_efficiency_zero_capital() {
        assert_eq!(compute_protocol_efficiency(500.0, 0.0), 0.0);
    }

    // ── Benchmark comparison ──────────────────────────────────────────────────

    #[test]
    fn benchmark_delta_positive_outperformance() {
        // strategy 12%, benchmark 4% → delta = 8%
        assert!((compute_benchmark_delta(0.12, 0.04) - 0.08).abs() < 1e-9);
    }

    #[test]
    fn benchmark_delta_underperformance() {
        // strategy 2%, benchmark 4% → delta = -2%
        assert!((compute_benchmark_delta(0.02, 0.04) - (-0.02)).abs() < 1e-9);
    }

    #[test]
    fn benchmark_delta_equal() {
        assert!((compute_benchmark_delta(0.04, 0.04)).abs() < 1e-9);
    }
}

// ── Integration tests ─────────────────────────────────────────────────────────

#[cfg(feature = "database")]
mod defi_analytics_integration_tests {
    use sqlx::PgPool;
    use std::sync::Arc;
    use uuid::Uuid;
    use chrono::Utc;

    use aframp_backend::defi::analytics::{
        DefiAnalyticsRepository, DefiAnalyticsService,
        models::{DefiPlatformSnapshot, DefiLendingSnapshot},
    };

    async fn setup_pool() -> PgPool {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/aframp_test".to_string());
        PgPool::connect(&url).await.expect("DB connect")
    }

    #[tokio::test]
    async fn test_platform_snapshot_insert_and_retrieve() {
        let pool = setup_pool().await;
        let repo = Arc::new(DefiAnalyticsRepository::new(Arc::new(pool)));

        let now = Utc::now();
        let snapshot = DefiPlatformSnapshot {
            snapshot_id: Uuid::new_v4(),
            snapshot_at: now,
            period_start: now - chrono::Duration::hours(1),
            period_end: now,
            total_value_locked: sqlx::types::BigDecimal::from(1_000_000i64),
            total_yield_distributed: sqlx::types::BigDecimal::from(5_000i64),
            weighted_avg_yield_rate: 0.08,
            total_amm_liquidity: sqlx::types::BigDecimal::from(300_000i64),
            total_collateral_locked: sqlx::types::BigDecimal::from(200_000i64),
            total_outstanding_loans: sqlx::types::BigDecimal::from(100_000i64),
            active_savings_positions: 50,
            active_amm_positions: 10,
            active_lending_positions: 5,
            platform_defi_revenue: sqlx::types::BigDecimal::from(1_000i64),
            created_at: now,
        };

        repo.insert_platform_snapshot(&snapshot).await.expect("insert snapshot");

        let retrieved = repo.get_latest_platform_snapshot().await.expect("get snapshot");
        assert!(retrieved.is_some());
        let s = retrieved.unwrap();
        assert_eq!(s.active_savings_positions, 50);
        assert!((s.weighted_avg_yield_rate - 0.08).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_lending_snapshot_insert_and_retrieve() {
        let pool = setup_pool().await;
        let repo = Arc::new(DefiAnalyticsRepository::new(Arc::new(pool)));

        let now = Utc::now();
        let snapshot = DefiLendingSnapshot {
            snapshot_id: Uuid::new_v4(),
            period_start: now - chrono::Duration::hours(24),
            period_end: now,
            total_collateral: sqlx::types::BigDecimal::from(500_000i64),
            total_outstanding_loans: sqlx::types::BigDecimal::from(200_000i64),
            avg_loan_to_value_ratio: 0.40,
            avg_health_factor: 1.8,
            liquidation_count: 2,
            liquidation_rate: 0.02,
            interest_income: sqlx::types::BigDecimal::from(1_000i64),
            unique_borrowers: 100,
            avg_loan_size: sqlx::types::BigDecimal::from(2_000i64),
            created_at: now,
        };

        repo.insert_lending_snapshot(&snapshot).await.expect("insert lending snapshot");

        let retrieved = repo.get_latest_lending_snapshot().await.expect("get lending snapshot");
        assert!(retrieved.is_some());
        let s = retrieved.unwrap();
        assert_eq!(s.liquidation_count, 2);
        assert!((s.avg_health_factor - 1.8).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_strategy_snapshot_insert_and_retrieve() {
        let pool = setup_pool().await;
        let repo = Arc::new(DefiAnalyticsRepository::new(Arc::new(pool)));

        let strategy_id = Uuid::new_v4();
        let now = Utc::now();

        let snapshot = aframp_backend::defi::analytics::models::DefiStrategySnapshot {
            snapshot_id: Uuid::new_v4(),
            strategy_id,
            period_start: now - chrono::Duration::hours(24),
            period_end: now,
            total_allocated: sqlx::types::BigDecimal::from(100_000i64),
            yield_earned: sqlx::types::BigDecimal::from(800i64),
            effective_yield_rate: 0.08,
            max_drawdown: 0.02,
            risk_adjusted_return: 0.08 / 1.02,
            rebalancing_event_count: 1,
            protocol_contributions: serde_json::json!({"stellar_amm": 0.6, "aave": 0.4}),
            benchmark_yield_rate: 0.04,
            benchmark_delta: 0.04,
            created_at: now,
        };

        repo.insert_strategy_snapshot(&snapshot).await.expect("insert strategy snapshot");

        let snapshots = repo.get_strategy_snapshots(strategy_id, 10).await.expect("get strategy snapshots");
        assert!(!snapshots.is_empty());
        assert!((snapshots[0].effective_yield_rate - 0.08).abs() < 1e-6);
        assert!((snapshots[0].benchmark_delta - 0.04).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_user_snapshot_upsert() {
        let pool = setup_pool().await;
        let repo = Arc::new(DefiAnalyticsRepository::new(Arc::new(pool)));

        let wallet_id = Uuid::new_v4();
        let now = Utc::now();
        let period_start = now - chrono::Duration::hours(24);

        let snapshot = aframp_backend::defi::analytics::models::DefiUserSnapshot {
            snapshot_id: Uuid::new_v4(),
            wallet_id,
            period_start,
            period_end: now,
            total_deposited_savings: sqlx::types::BigDecimal::from(10_000i64),
            total_yield_earned: sqlx::types::BigDecimal::from(80i64),
            net_yield_rate: 0.08,
            total_collateral_locked: sqlx::types::BigDecimal::from(5_000i64),
            outstanding_loan_balance: sqlx::types::BigDecimal::from(2_000i64),
            net_defi_position_value: sqlx::types::BigDecimal::from(13_000i64),
            product_usage: serde_json::json!({"savings": 1, "lending": 1}),
            created_at: now,
        };

        repo.upsert_user_snapshot(&snapshot).await.expect("upsert user snapshot");

        let retrieved = repo.get_user_latest_snapshot(wallet_id).await.expect("get user snapshot");
        assert!(retrieved.is_some());
        let s = retrieved.unwrap();
        assert_eq!(s.wallet_id, wallet_id);
        assert!((s.net_yield_rate - 0.08).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_report_generation() {
        let pool = setup_pool().await;
        let repo = Arc::new(DefiAnalyticsRepository::new(Arc::new(pool.clone())));
        let svc = Arc::new(DefiAnalyticsService::new(repo));

        let report = svc.generate_report("weekly").await.expect("generate report");
        assert_eq!(report.report_type, "weekly");
        // After generation the status should be 'ready'
        let reports = svc.list_reports().await.expect("list reports");
        let found = reports.iter().find(|r| r.report_id == report.report_id);
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_analytics_caching_invalidation() {
        // Verify that two consecutive platform snapshots are both persisted
        let pool = setup_pool().await;
        let repo = Arc::new(DefiAnalyticsRepository::new(Arc::new(pool)));

        let now = Utc::now();
        for i in 0..2i64 {
            let s = DefiPlatformSnapshot {
                snapshot_id: Uuid::new_v4(),
                snapshot_at: now + chrono::Duration::seconds(i),
                period_start: now - chrono::Duration::hours(1),
                period_end: now,
                total_value_locked: sqlx::types::BigDecimal::from(1_000_000i64 + i * 1000),
                total_yield_distributed: sqlx::types::BigDecimal::from(5_000i64),
                weighted_avg_yield_rate: 0.08,
                total_amm_liquidity: sqlx::types::BigDecimal::from(300_000i64),
                total_collateral_locked: sqlx::types::BigDecimal::from(200_000i64),
                total_outstanding_loans: sqlx::types::BigDecimal::from(100_000i64),
                active_savings_positions: 50,
                active_amm_positions: 10,
                active_lending_positions: 5,
                platform_defi_revenue: sqlx::types::BigDecimal::from(1_000i64),
                created_at: now,
            };
            repo.insert_platform_snapshot(&s).await.expect("insert");
        }

        let history = repo.get_platform_snapshot_history(10).await.expect("history");
        assert!(history.len() >= 2);
    }
}
