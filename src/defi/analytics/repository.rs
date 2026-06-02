use std::sync::Arc;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use super::models::*;

pub struct DefiAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl DefiAnalyticsRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    // ── Platform Snapshots ────────────────────────────────────────────────────

    pub async fn insert_platform_snapshot(&self, s: &DefiPlatformSnapshot) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_platform_snapshots (
                snapshot_id, snapshot_at, period_start, period_end,
                total_value_locked, total_yield_distributed, weighted_avg_yield_rate,
                total_amm_liquidity, total_collateral_locked, total_outstanding_loans,
                active_savings_positions, active_amm_positions, active_lending_positions,
                platform_defi_revenue
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            "#,
            s.snapshot_id, s.snapshot_at, s.period_start, s.period_end,
            s.total_value_locked, s.total_yield_distributed, s.weighted_avg_yield_rate,
            s.total_amm_liquidity, s.total_collateral_locked, s.total_outstanding_loans,
            s.active_savings_positions, s.active_amm_positions, s.active_lending_positions,
            s.platform_defi_revenue,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_latest_platform_snapshot(&self) -> Result<Option<DefiPlatformSnapshot>, AppError> {
        let row = sqlx::query_as!(
            DefiPlatformSnapshot,
            "SELECT * FROM defi_platform_snapshots ORDER BY snapshot_at DESC LIMIT 1"
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_platform_snapshot_history(
        &self,
        limit: i64,
    ) -> Result<Vec<DefiPlatformSnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiPlatformSnapshot,
            "SELECT * FROM defi_platform_snapshots ORDER BY snapshot_at DESC LIMIT $1",
            limit
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    // ── Strategy Snapshots ────────────────────────────────────────────────────

    pub async fn insert_strategy_snapshot(&self, s: &DefiStrategySnapshot) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_strategy_snapshots (
                snapshot_id, strategy_id, period_start, period_end,
                total_allocated, yield_earned, effective_yield_rate,
                max_drawdown, risk_adjusted_return, rebalancing_event_count,
                protocol_contributions, benchmark_yield_rate, benchmark_delta
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            "#,
            s.snapshot_id, s.strategy_id, s.period_start, s.period_end,
            s.total_allocated, s.yield_earned, s.effective_yield_rate,
            s.max_drawdown, s.risk_adjusted_return, s.rebalancing_event_count,
            s.protocol_contributions, s.benchmark_yield_rate, s.benchmark_delta,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_strategy_snapshots(
        &self,
        strategy_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DefiStrategySnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiStrategySnapshot,
            "SELECT * FROM defi_strategy_snapshots WHERE strategy_id = $1 ORDER BY period_start DESC LIMIT $2",
            strategy_id, limit
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_all_strategy_latest_snapshots(&self) -> Result<Vec<DefiStrategySnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiStrategySnapshot,
            r#"
            SELECT DISTINCT ON (strategy_id) *
            FROM defi_strategy_snapshots
            ORDER BY strategy_id, period_start DESC
            "#
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    // ── Protocol Snapshots ────────────────────────────────────────────────────

    pub async fn insert_protocol_snapshot(&self, s: &DefiProtocolSnapshot) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_protocol_snapshots (
                snapshot_id, protocol_id, period_start, period_end,
                platform_exposure, yield_earned, fee_income, impermanent_loss,
                health_score, uptime_pct, capital_efficiency
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            "#,
            s.snapshot_id, s.protocol_id, s.period_start, s.period_end,
            s.platform_exposure, s.yield_earned, s.fee_income, s.impermanent_loss,
            s.health_score, s.uptime_pct, s.capital_efficiency,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_protocol_snapshots(
        &self,
        protocol_id: &str,
        limit: i64,
    ) -> Result<Vec<DefiProtocolSnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiProtocolSnapshot,
            "SELECT * FROM defi_protocol_snapshots WHERE protocol_id = $1 ORDER BY period_start DESC LIMIT $2",
            protocol_id, limit
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_all_protocol_latest_snapshots(&self) -> Result<Vec<DefiProtocolSnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiProtocolSnapshot,
            r#"
            SELECT DISTINCT ON (protocol_id) *
            FROM defi_protocol_snapshots
            ORDER BY protocol_id, period_start DESC
            "#
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    // ── AMM Pool Snapshots ────────────────────────────────────────────────────

    pub async fn insert_amm_pool_snapshot(&self, s: &DefiAmmPoolSnapshot) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_amm_pool_snapshots (
                snapshot_id, pool_id, period_start, period_end,
                trading_volume, fee_income, impermanent_loss, hold_strategy_return,
                actual_yield, capital_efficiency, price_range_coverage_pct
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            "#,
            s.snapshot_id, s.pool_id, s.period_start, s.period_end,
            s.trading_volume, s.fee_income, s.impermanent_loss, s.hold_strategy_return,
            s.actual_yield, s.capital_efficiency, s.price_range_coverage_pct,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_amm_pool_snapshots(
        &self,
        pool_id: &str,
        limit: i64,
    ) -> Result<Vec<DefiAmmPoolSnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiAmmPoolSnapshot,
            "SELECT * FROM defi_amm_pool_snapshots WHERE pool_id = $1 ORDER BY period_start DESC LIMIT $2",
            pool_id, limit
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_all_amm_pool_latest_snapshots(&self) -> Result<Vec<DefiAmmPoolSnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiAmmPoolSnapshot,
            r#"
            SELECT DISTINCT ON (pool_id) *
            FROM defi_amm_pool_snapshots
            ORDER BY pool_id, period_start DESC
            "#
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    // ── Lending Snapshots ─────────────────────────────────────────────────────

    pub async fn insert_lending_snapshot(&self, s: &DefiLendingSnapshot) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_lending_snapshots (
                snapshot_id, period_start, period_end,
                total_collateral, total_outstanding_loans, avg_loan_to_value_ratio,
                avg_health_factor, liquidation_count, liquidation_rate,
                interest_income, unique_borrowers, avg_loan_size
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            "#,
            s.snapshot_id, s.period_start, s.period_end,
            s.total_collateral, s.total_outstanding_loans, s.avg_loan_to_value_ratio,
            s.avg_health_factor, s.liquidation_count, s.liquidation_rate,
            s.interest_income, s.unique_borrowers, s.avg_loan_size,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_latest_lending_snapshot(&self) -> Result<Option<DefiLendingSnapshot>, AppError> {
        let row = sqlx::query_as!(
            DefiLendingSnapshot,
            "SELECT * FROM defi_lending_snapshots ORDER BY period_start DESC LIMIT 1"
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_lending_snapshot_history(&self, limit: i64) -> Result<Vec<DefiLendingSnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiLendingSnapshot,
            "SELECT * FROM defi_lending_snapshots ORDER BY period_start DESC LIMIT $1",
            limit
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    // ── User Snapshots ────────────────────────────────────────────────────────

    pub async fn upsert_user_snapshot(&self, s: &DefiUserSnapshot) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_user_snapshots (
                snapshot_id, wallet_id, period_start, period_end,
                total_deposited_savings, total_yield_earned, net_yield_rate,
                total_collateral_locked, outstanding_loan_balance,
                net_defi_position_value, product_usage
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            ON CONFLICT (wallet_id, period_start) DO UPDATE SET
                total_deposited_savings = EXCLUDED.total_deposited_savings,
                total_yield_earned = EXCLUDED.total_yield_earned,
                net_yield_rate = EXCLUDED.net_yield_rate,
                total_collateral_locked = EXCLUDED.total_collateral_locked,
                outstanding_loan_balance = EXCLUDED.outstanding_loan_balance,
                net_defi_position_value = EXCLUDED.net_defi_position_value,
                product_usage = EXCLUDED.product_usage
            "#,
            s.snapshot_id, s.wallet_id, s.period_start, s.period_end,
            s.total_deposited_savings, s.total_yield_earned, s.net_yield_rate,
            s.total_collateral_locked, s.outstanding_loan_balance,
            s.net_defi_position_value, s.product_usage,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_latest_snapshot(&self, wallet_id: Uuid) -> Result<Option<DefiUserSnapshot>, AppError> {
        let row = sqlx::query_as!(
            DefiUserSnapshot,
            "SELECT * FROM defi_user_snapshots WHERE wallet_id = $1 ORDER BY period_start DESC LIMIT 1",
            wallet_id
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_user_yield_history(
        &self,
        wallet_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DefiUserSnapshot>, AppError> {
        let rows = sqlx::query_as!(
            DefiUserSnapshot,
            "SELECT * FROM defi_user_snapshots WHERE wallet_id = $1 ORDER BY period_start DESC LIMIT $2",
            wallet_id, limit
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    // ── Reports ───────────────────────────────────────────────────────────────

    pub async fn insert_report(&self, r: &DefiAnalyticsReport) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_analytics_reports (
                report_id, report_type, period_start, period_end, status,
                report_data, download_url, generated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
            r.report_id, r.report_type, r.period_start, r.period_end, r.status,
            r.report_data, r.download_url, r.generated_at,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_report_ready(
        &self,
        report_id: Uuid,
        data: serde_json::Value,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE defi_analytics_reports
            SET status = 'ready', report_data = $2, generated_at = NOW()
            WHERE report_id = $1
            "#,
            report_id, data,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_reports(&self) -> Result<Vec<DefiAnalyticsReport>, AppError> {
        let rows = sqlx::query_as!(
            DefiAnalyticsReport,
            "SELECT * FROM defi_analytics_reports ORDER BY created_at DESC LIMIT 100"
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    // ── Aggregation helpers ───────────────────────────────────────────────────

    /// Total savings deposits across all active accounts
    pub async fn sum_savings_deposits(&self) -> Result<sqlx::types::BigDecimal, AppError> {
        let row = sqlx::query!(
            "SELECT COALESCE(SUM(deposited_amount), 0) AS total FROM cngn_savings_accounts WHERE account_status = 'active'"
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.total.unwrap_or_default())
    }

    /// Total AMM liquidity across all active positions
    pub async fn sum_amm_liquidity(&self) -> Result<sqlx::types::BigDecimal, AppError> {
        let row = sqlx::query!(
            "SELECT COALESCE(SUM(asset_a_deposited + asset_b_deposited), 0) AS total FROM amm_liquidity_positions WHERE position_status = 'active'"
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.total.unwrap_or_default())
    }

    /// Total collateral locked in lending
    pub async fn sum_lending_collateral(&self) -> Result<sqlx::types::BigDecimal, AppError> {
        let row = sqlx::query!(
            "SELECT COALESCE(SUM(collateral_amount), 0) AS total FROM lending_positions WHERE status = 'active'"
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.total.unwrap_or_default())
    }

    /// Total outstanding loans
    pub async fn sum_outstanding_loans(&self) -> Result<sqlx::types::BigDecimal, AppError> {
        let row = sqlx::query!(
            "SELECT COALESCE(SUM(borrowed_amount), 0) AS total FROM lending_positions WHERE status IN ('active', 'at_risk')"
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.total.unwrap_or_default())
    }

    /// Yield distributed in period
    pub async fn sum_yield_in_period(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<sqlx::types::BigDecimal, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT COALESCE(SUM(yield_amount_earned), 0) AS total
            FROM yield_accrual_entries
            WHERE accrual_timestamp >= $1 AND accrual_timestamp < $2
            "#,
            period_start, period_end
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.total.unwrap_or_default())
    }

    /// Weighted average yield rate across active savings products
    pub async fn weighted_avg_yield_rate(&self) -> Result<f64, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT
                CASE WHEN SUM(a.deposited_amount) = 0 THEN 0
                ELSE SUM(a.current_yield_rate * a.deposited_amount::float8) / SUM(a.deposited_amount::float8)
                END AS wavg
            FROM cngn_savings_accounts a
            WHERE a.account_status = 'active'
            "#
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.wavg.unwrap_or(0.0))
    }

    /// Count active positions per category
    pub async fn count_active_positions(&self) -> Result<(i64, i64, i64), AppError> {
        let savings = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM cngn_savings_accounts WHERE account_status = 'active'"
        )
        .fetch_one(&*self.pool)
        .await?
        .unwrap_or(0);

        let amm = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM amm_liquidity_positions WHERE position_status = 'active'"
        )
        .fetch_one(&*self.pool)
        .await?
        .unwrap_or(0);

        let lending = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM lending_positions WHERE status IN ('active', 'at_risk')"
        )
        .fetch_one(&*self.pool)
        .await?
        .unwrap_or(0);

        Ok((savings, amm, lending))
    }

    /// Rebalancing event count for a strategy in a period
    pub async fn count_rebalancing_events(
        &self,
        strategy_id: Uuid,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM rebalancing_events
            WHERE strategy_id = $1 AND started_at >= $2 AND started_at < $3
            "#,
            strategy_id, period_start, period_end
        )
        .fetch_one(&*self.pool)
        .await?
        .unwrap_or(0);
        Ok(count)
    }

    /// Liquidation count in period
    pub async fn count_liquidations_in_period(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM liquidation_events WHERE liquidated_at >= $1 AND liquidated_at < $2",
            period_start, period_end
        )
        .fetch_one(&*self.pool)
        .await?
        .unwrap_or(0);
        Ok(count)
    }

    /// Average health factor across active lending positions
    pub async fn avg_lending_health_factor(&self) -> Result<f64, AppError> {
        let row = sqlx::query_scalar!(
            "SELECT AVG(health_factor::float8) FROM lending_positions WHERE status = 'active'"
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.flatten().unwrap_or(0.0))
    }

    /// Unique borrowers count
    pub async fn count_unique_borrowers(&self) -> Result<i64, AppError> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(DISTINCT wallet_id) FROM lending_positions WHERE status IN ('active', 'at_risk', 'repaid')"
        )
        .fetch_one(&*self.pool)
        .await?
        .unwrap_or(0);
        Ok(count)
    }

    /// User savings data for personal analytics
    pub async fn get_user_savings_data(
        &self,
        wallet_id: Uuid,
    ) -> Result<(sqlx::types::BigDecimal, sqlx::types::BigDecimal, f64), AppError> {
        let row = sqlx::query!(
            r#"
            SELECT
                COALESCE(SUM(deposited_amount), 0) AS total_deposited,
                COALESCE(SUM(accrued_yield_to_date), 0) AS total_yield,
                CASE WHEN SUM(deposited_amount) = 0 THEN 0
                ELSE SUM(current_yield_rate * deposited_amount::float8) / SUM(deposited_amount::float8)
                END AS net_yield_rate
            FROM cngn_savings_accounts
            WHERE wallet_id = $1 AND account_status = 'active'
            "#,
            wallet_id
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok((
            row.total_deposited.unwrap_or_default(),
            row.total_yield.unwrap_or_default(),
            row.net_yield_rate.unwrap_or(0.0),
        ))
    }

    /// User lending data for personal analytics
    pub async fn get_user_lending_data(
        &self,
        wallet_id: Uuid,
    ) -> Result<(sqlx::types::BigDecimal, sqlx::types::BigDecimal), AppError> {
        let row = sqlx::query!(
            r#"
            SELECT
                COALESCE(SUM(collateral_amount), 0) AS total_collateral,
                COALESCE(SUM(borrowed_amount), 0) AS total_loans
            FROM lending_positions
            WHERE wallet_id = $1 AND status IN ('active', 'at_risk')
            "#,
            wallet_id
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok((
            row.total_collateral.unwrap_or_default(),
            row.total_loans.unwrap_or_default(),
        ))
    }

    /// Insert export request
    pub async fn insert_export_request(
        &self,
        export_id: Uuid,
        requester_id: &str,
        scope: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        metrics: &serde_json::Value,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_export_requests (export_id, requester_id, export_scope, date_range_start, date_range_end, metric_set)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            export_id, requester_id, scope, start, end, metrics,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }
}
