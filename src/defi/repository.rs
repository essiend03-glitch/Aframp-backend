use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::database::DbPool;
use crate::error::AppError;
use super::{
    YieldStrategy, StrategyAllocation, StrategyRiskParameters, StrategyPerformance,
    GovernanceApprovalRecord, GovernanceApproval, ProtocolConfig, DeFiPosition,
    CngnSavingsProduct, CngnSavingsAccount, YieldAccrualRecord, WithdrawalRequest,
    StellarAmmPool, AmmLiquidityPosition, CircuitBreakerTrip, RebalancingEvent,
    YieldRateHistory, TreasuryExposureMetrics, ProtocolExposure,
};

/// Repository layer for DeFi database operations
pub struct DeFiRepository {
    db_pool: Arc<DbPool>,
}

impl DeFiRepository {
    pub fn new(db_pool: Arc<DbPool>) -> Self {
        Self { db_pool }
    }

    // ── Strategy Management ─────────────────────────────────────────────────────

    /// Create a new yield strategy
    pub async fn create_strategy(&self, strategy: &YieldStrategy) -> Result<Uuid, AppError> {
        let strategy_id = sqlx::query!(
            r#"
            INSERT INTO yield_strategies (
                strategy_id, strategy_name, description, strategy_type,
                target_yield_rate, min_acceptable_yield_rate, max_acceptable_risk_score,
                total_allocated_amount, max_allocation_limit, rebalancing_frequency_secs,
                rebalancing_triggers, strategy_status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING strategy_id
            "#,
            strategy.strategy_id,
            strategy.strategy_name,
            strategy.description,
            strategy.strategy_type as super::StrategyType,
            strategy.target_yield_rate,
            strategy.min_acceptable_yield_rate,
            strategy.max_acceptable_risk_score,
            strategy.total_allocated_amount,
            strategy.max_allocation_limit,
            strategy.rebalancing_frequency_secs,
            serde_json::to_value(&strategy.rebalancing_triggers)?,
            strategy.strategy_status as super::StrategyStatus,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(strategy_id)
    }

    /// Get strategy by ID
    pub async fn get_strategy(&self, strategy_id: Uuid) -> Result<Option<YieldStrategy>, AppError> {
        let strategy = sqlx::query_as_opt!(
            YieldStrategy,
            r#"
            SELECT 
                strategy_id, strategy_name, description, strategy_type as "strategy_type: super::StrategyType",
                target_yield_rate, min_acceptable_yield_rate, max_acceptable_risk_score,
                total_allocated_amount, max_allocation_limit, rebalancing_frequency_secs,
                rebalancing_triggers, strategy_status as "strategy_status: super::StrategyStatus",
                created_at, updated_at
            FROM yield_strategies
            WHERE strategy_id = $1
            "#,
            strategy_id
        )
        .await?;

        Ok(strategy)
    }

    /// List strategies with optional filtering
    pub async fn list_strategies(
        &self,
        status: Option<super::StrategyStatus>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<YieldStrategy>, AppError> {
        let mut query = "
            SELECT 
                strategy_id, strategy_name, description, strategy_type,
                target_yield_rate, min_acceptable_yield_rate, max_acceptable_risk_score,
                total_allocated_amount, max_allocation_limit, rebalancing_frequency_secs,
                rebalancing_triggers, strategy_status, created_at, updated_at
            FROM yield_strategies
            WHERE 1=1
        ".to_string();

        if let Some(status) = status {
            query.push_str(&format!(" AND strategy_status = '{}'", 
                serde_json::to_string(&status)?.trim_matches('"')));
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        let strategies = sqlx::query_as!(
            YieldStrategy,
            &query
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(strategies)
    }

    /// Update strategy status
    pub async fn update_strategy_status(&self, strategy_id: Uuid, status: super::StrategyStatus) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE yield_strategies SET strategy_status = $1, updated_at = NOW() WHERE strategy_id = $2",
            status as super::StrategyStatus,
            strategy_id
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    /// Update strategy allocation amount
    pub async fn update_strategy_allocation(&self, strategy_id: Uuid, allocated_amount: BigDecimal) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE yield_strategies SET total_allocated_amount = $1, updated_at = NOW() WHERE strategy_id = $2",
            allocated_amount,
            strategy_id
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    // ── Strategy Allocations ───────────────────────────────────────────────────

    /// Create strategy allocation
    pub async fn create_allocation(&self, allocation: &StrategyAllocation) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO strategy_allocations (
                allocation_id, strategy_id, protocol_id, target_allocation_percentage,
                current_allocation_amount, min_allocation_percentage, max_allocation_percentage,
                last_rebalanced_at, allocation_status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING allocation_id
            "#,
            allocation.allocation_id,
            allocation.strategy_id,
            allocation.protocol_id,
            allocation.target_allocation_percentage,
            allocation.current_allocation_amount,
            allocation.min_allocation_percentage,
            allocation.max_allocation_percentage,
            allocation.last_rebalanced_at,
            allocation.allocation_status as super::AllocationStatus,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(allocation.allocation_id)
    }

    /// Get strategy allocations
    pub async fn get_strategy_allocations(&self, strategy_id: Uuid) -> Result<Vec<StrategyAllocation>, AppError> {
        let allocations = sqlx::query_as!(
            StrategyAllocation,
            r#"
            SELECT 
                allocation_id, strategy_id, protocol_id, target_allocation_percentage,
                current_allocation_amount, min_allocation_percentage, max_allocation_percentage,
                last_rebalanced_at, allocation_status as "allocation_status: super::AllocationStatus"
            FROM strategy_allocations
            WHERE strategy_id = $1
            "#,
            strategy_id
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(allocations)
    }

    /// Update allocation amount
    pub async fn update_allocation_amount(&self, allocation_id: Uuid, amount: BigDecimal) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE strategy_allocations 
            SET current_allocation_amount = $1, last_rebalanced_at = NOW()
            WHERE allocation_id = $2
            "#,
            amount,
            allocation_id
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    // ── Strategy Risk Parameters ───────────────────────────────────────────────

    /// Create strategy risk parameters
    pub async fn create_risk_parameters(&self, params: &StrategyRiskParameters) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO strategy_risk_parameters (
                parameter_id, strategy_id, max_single_protocol_exposure_pct,
                max_correlation_between_protocols, max_acceptable_impermanent_loss_pct,
                circuit_breaker_tvl_drop_threshold, emergency_withdrawal_trigger_conditions
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING parameter_id
            "#,
            params.parameter_id,
            params.strategy_id,
            params.max_single_protocol_exposure_pct,
            params.max_correlation_between_protocols,
            params.max_acceptable_impermanent_loss_pct,
            params.circuit_breaker_tvl_drop_threshold,
            params.emergency_withdrawal_trigger_conditions,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(params.parameter_id)
    }

    /// Get strategy risk parameters
    pub async fn get_risk_parameters(&self, strategy_id: Uuid) -> Result<Option<StrategyRiskParameters>, AppError> {
        let params = sqlx::query_as_opt!(
            StrategyRiskParameters,
            r#"
            SELECT 
                parameter_id, strategy_id, max_single_protocol_exposure_pct,
                max_correlation_between_protocols, max_acceptable_impermanent_loss_pct,
                circuit_breaker_tvl_drop_threshold, emergency_withdrawal_trigger_conditions,
                created_at, updated_at
            FROM strategy_risk_parameters
            WHERE strategy_id = $1
            "#,
            strategy_id
        )
        .await?;

        Ok(params)
    }

    // ── Strategy Performance ───────────────────────────────────────────────────

    /// Create strategy performance record
    pub async fn create_performance_record(&self, performance: &StrategyPerformance) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO strategy_performance (
                performance_id, strategy_id, period_start, period_end,
                opening_allocation, closing_allocation, yield_earned,
                effective_yield_rate, max_drawdown, risk_score_at_end
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING performance_id
            "#,
            performance.performance_id,
            performance.strategy_id,
            performance.period_start,
            performance.period_end,
            performance.opening_allocation,
            performance.closing_allocation,
            performance.yield_earned,
            performance.effective_yield_rate,
            performance.max_drawdown,
            performance.risk_score_at_end,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(performance.performance_id)
    }

    /// Get strategy performance history
    pub async fn get_strategy_performance(&self, strategy_id: Uuid, limit: Option<i64>) -> Result<Vec<StrategyPerformance>, AppError> {
        let mut query = "
            SELECT 
                performance_id, strategy_id, period_start, period_end,
                opening_allocation, closing_allocation, yield_earned,
                effective_yield_rate, max_drawdown, risk_score_at_end, created_at
            FROM strategy_performance
            WHERE strategy_id = $1
            ORDER BY period_start DESC
        ".to_string();

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let performance = sqlx::query_as!(
            StrategyPerformance,
            &query,
            strategy_id
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(performance)
    }

    // ── Governance Approvals ─────────────────────────────────────────────────────

    /// Create governance approval record
    pub async fn create_governance_approval(&self, approval: &GovernanceApprovalRecord) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO strategy_governance_approvals (
                record_id, strategy_id, submitted_by, submitted_at,
                required_approvals, received_approvals, approval_status, rejection_reason
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (strategy_id) DO UPDATE SET
                submitted_by = EXCLUDED.submitted_by,
                submitted_at = EXCLUDED.submitted_at,
                required_approvals = EXCLUDED.required_approvals,
                received_approvals = EXCLUDED.received_approvals,
                approval_status = EXCLUDED.approval_status,
                rejection_reason = EXCLUDED.rejection_reason
            RETURNING record_id
            "#,
            approval.record_id,
            approval.strategy_id,
            approval.submitted_by,
            approval.submitted_at,
            approval.required_approvals as i32,
            approval.received_approvals as i32,
            approval.approval_status as super::GovernanceStatus,
            approval.rejection_reason,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(approval.record_id)
    }

    /// Get governance approval record
    pub async fn get_governance_approval(&self, strategy_id: Uuid) -> Result<Option<GovernanceApprovalRecord>, AppError> {
        let record = sqlx::query_as_opt!(
            GovernanceApprovalRecord,
            r#"
            SELECT 
                record_id, strategy_id, submitted_by, submitted_at,
                required_approvals, received_approvals, approval_status as "approval_status: super::GovernanceStatus",
                rejection_reason
            FROM strategy_governance_approvals
            WHERE strategy_id = $1
            "#,
            strategy_id
        )
        .await?;

        if let Some(record) = record {
            // Get individual approvals
            let approvals = sqlx::query_as!(
                GovernanceApproval,
                r#"
                SELECT 
                    approval_id, committee_member, approved_at, justification,
                    approval_type as "approval_type: super::ApprovalType"
                FROM governance_approvals
                WHERE record_id = $1
                "#,
                record.record_id
            )
            .fetch_all(&*self.db_pool)
            .await?;

            Ok(Some(GovernanceApprovalRecord {
                record_id: record.record_id,
                strategy_id: record.strategy_id,
                submitted_by: record.submitted_by,
                submitted_at: record.submitted_at,
                required_approvals: record.required_approvals as usize,
                received_approvals: record.received_approvals as usize,
                approval_status: record.approval_status,
                approvals,
                rejection_reason: record.rejection_reason,
            }))
        } else {
            Ok(None)
        }
    }

    /// Create individual approval
    pub async fn create_approval(&self, approval: &GovernanceApproval, record_id: Uuid) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO governance_approvals (
                approval_id, record_id, committee_member, approved_at,
                justification, approval_type
            ) VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING approval_id
            "#,
            approval.approval_id,
            record_id,
            approval.committee_member,
            approval.approved_at,
            approval.justification,
            approval.approval_type as super::ApprovalType,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(approval.approval_id)
    }

    // ── DeFi Positions ─────────────────────────────────────────────────────────

    /// Create DeFi position
    pub async fn create_defi_position(&self, position: &DeFiPosition) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_positions (
                position_id, protocol_id, asset_code, deposited_amount, current_value,
                yield_earned, effective_yield_rate, position_opened_at, last_updated_at,
                position_status, protocol_position_id, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING position_id
            "#,
            position.position_id,
            position.protocol_id,
            position.asset_code,
            position.deposited_amount,
            position.current_value,
            position.yield_earned,
            position.effective_yield_rate,
            position.position_opened_at,
            position.last_updated_at,
            position.position_status as super::PositionStatus,
            position.protocol_position_id,
            serde_json::to_value(HashMap::<String, serde_json::Value>::new())?,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(position.position_id)
    }

    /// Get DeFi position
    pub async fn get_defi_position(&self, position_id: Uuid) -> Result<Option<DeFiPosition>, AppError> {
        let position = sqlx::query_as_opt!(
            DeFiPosition,
            r#"
            SELECT 
                position_id, protocol_id, asset_code, deposited_amount, current_value,
                yield_earned, effective_yield_rate, position_opened_at, last_updated_at,
                position_status as "position_status: super::PositionStatus"
            FROM defi_positions
            WHERE position_id = $1
            "#,
            position_id
        )
        .await?;

        Ok(position)
    }

    /// Update DeFi position
    pub async fn update_defi_position(&self, position: &DeFiPosition) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE defi_positions 
            SET current_value = $1, yield_earned = $2, effective_yield_rate = $3,
                last_updated_at = $4, position_status = $5
            WHERE position_id = $6
            "#,
            position.current_value,
            position.yield_earned,
            position.effective_yield_rate,
            position.last_updated_at,
            position.position_status as super::PositionStatus,
            position.position_id,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    /// Get positions by protocol
    pub async fn get_positions_by_protocol(&self, protocol_id: &str) -> Result<Vec<DeFiPosition>, AppError> {
        let positions = sqlx::query_as!(
            DeFiPosition,
            r#"
            SELECT 
                position_id, protocol_id, asset_code, deposited_amount, current_value,
                yield_earned, effective_yield_rate, position_opened_at, last_updated_at,
                position_status as "position_status: super::PositionStatus"
            FROM defi_positions
            WHERE protocol_id = $1 AND position_status = 'active'
            "#,
            protocol_id
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(positions)
    }

    // ── cNGN Savings Products ─────────────────────────────────────────────────

    /// Create savings product
    pub async fn create_savings_product(&self, product: &CngnSavingsProduct) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO cngn_savings_products (
                product_id, product_name, description, product_type,
                minimum_deposit_amount, maximum_deposit_amount, lock_up_period_hours,
                early_withdrawal_penalty_pct, target_yield_rate, yield_rate_source,
                underlying_strategy_id, yield_rate_floor, yield_rate_ceil,
                product_status, risk_disclosure_version
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING product_id
            "#,
            product.product_id,
            product.product_name,
            product.description,
            product.product_type as super::SavingsProductType,
            product.minimum_deposit_amount,
            product.maximum_deposit_amount,
            product.lock_up_period_hours,
            product.early_withdrawal_penalty_pct,
            product.target_yield_rate,
            product.yield_rate_source,
            product.underlying_strategy_id,
            product.yield_rate_floor,
            product.yield_rate_ceil,
            product.product_status,
            product.risk_disclosure_version,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(product.product_id)
    }

    /// Get savings product
    pub async fn get_savings_product(&self, product_id: Uuid) -> Result<Option<CngnSavingsProduct>, AppError> {
        let product = sqlx::query_as_opt!(
            CngnSavingsProduct,
            r#"
            SELECT 
                product_id, product_name, description, product_type as "product_type: super::SavingsProductType",
                minimum_deposit_amount, maximum_deposit_amount, lock_up_period_hours,
                early_withdrawal_penalty_pct, target_yield_rate, yield_rate_source,
                underlying_strategy_id, yield_rate_floor, yield_rate_ceil,
                product_status, risk_disclosure_version, created_at, updated_at
            FROM cngn_savings_products
            WHERE product_id = $1
            "#,
            product_id
        )
        .await?;

        Ok(product)
    }

    /// List savings products
    pub async fn list_savings_products(&self, active_only: bool) -> Result<Vec<CngnSavingsProduct>, AppError> {
        let mut query = "
            SELECT 
                product_id, product_name, description, product_type,
                minimum_deposit_amount, maximum_deposit_amount, lock_up_period_hours,
                early_withdrawal_penalty_pct, target_yield_rate, yield_rate_source,
                underlying_strategy_id, yield_rate_floor, yield_rate_ceil,
                product_status, risk_disclosure_version, created_at, updated_at
            FROM cngn_savings_products
        ".to_string();

        if active_only {
            query.push_str(" WHERE product_status = 'active'");
        }

        query.push_str(" ORDER BY created_at DESC");

        let products = sqlx::query_as!(
            CngnSavingsProduct,
            &query
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(products)
    }

    // ── cNGN Savings Accounts ─────────────────────────────────────────────────

    /// Create savings account
    pub async fn create_savings_account(&self, account: &CngnSavingsAccount) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO cngn_savings_accounts (
                account_id, wallet_id, product_id, deposited_amount, current_balance,
                accrued_yield_to_date, current_yield_rate, deposit_timestamp,
                last_yield_accrual_timestamp, withdrawal_eligibility_timestamp,
                account_status, risk_disclosure_accepted_at, risk_disclosure_ip_address
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING account_id
            "#,
            account.account_id,
            account.wallet_id,
            account.product_id,
            account.deposited_amount,
            account.current_balance,
            account.accrued_yield_to_date,
            account.current_yield_rate,
            account.deposit_timestamp,
            account.last_yield_accrual_timestamp,
            account.withdrawal_eligibility_timestamp,
            account.account_status as super::SavingsAccountStatus,
            account.risk_disclosure_accepted_at,
            account.risk_disclosure_ip_address,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(account.account_id)
    }

    /// Get savings account
    pub async fn get_savings_account(&self, account_id: Uuid) -> Result<Option<CngnSavingsAccount>, AppError> {
        let account = sqlx::query_as_opt!(
            CngnSavingsAccount,
            r#"
            SELECT 
                account_id, wallet_id, product_id, deposited_amount, current_balance,
                accrued_yield_to_date, current_yield_rate, deposit_timestamp,
                last_yield_accrual_timestamp, withdrawal_eligibility_timestamp,
                account_status as "account_status: super::SavingsAccountStatus",
                risk_disclosure_accepted_at, risk_disclosure_ip_address
            FROM cngn_savings_accounts
            WHERE account_id = $1
            "#,
            account_id
        )
        .await?;

        Ok(account)
    }

    /// Get savings accounts by wallet
    pub async fn get_wallet_savings_accounts(&self, wallet_id: Uuid) -> Result<Vec<CngnSavingsAccount>, AppError> {
        let accounts = sqlx::query_as!(
            CngnSavingsAccount,
            r#"
            SELECT 
                account_id, wallet_id, product_id, deposited_amount, current_balance,
                accrued_yield_to_date, current_yield_rate, deposit_timestamp,
                last_yield_accrual_timestamp, withdrawal_eligibility_timestamp,
                account_status as "account_status: super::SavingsAccountStatus",
                risk_disclosure_accepted_at, risk_disclosure_ip_address
            FROM cngn_savings_accounts
            WHERE wallet_id = $1
            ORDER BY created_at DESC
            "#,
            wallet_id
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(accounts)
    }

    /// Update savings account balance
    pub async fn update_savings_account_balance(
        &self,
        account_id: Uuid,
        current_balance: BigDecimal,
        accrued_yield: BigDecimal,
        yield_rate: f64,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE cngn_savings_accounts 
            SET current_balance = $1, accrued_yield_to_date = $2, current_yield_rate = $3,
                last_yield_accrual_timestamp = NOW()
            WHERE account_id = $4
            "#,
            current_balance,
            accrued_yield,
            yield_rate,
            account_id
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    // ── Yield Accrual Records ─────────────────────────────────────────────────

    /// Create yield accrual record
    pub async fn create_yield_accrual(&self, accrual: &YieldAccrualRecord) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO yield_accrual_records (
                accrual_id, account_id, accrual_period_start, accrual_period_end,
                opening_balance, yield_rate_applied, yield_amount_earned, accrual_timestamp
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING accrual_id
            "#,
            accrual.accrual_id,
            accrual.account_id,
            accrual.accrual_period_start,
            accrual.accrual_period_end,
            accrual.opening_balance,
            accrual.yield_rate_applied,
            accrual.yield_amount_earned,
            accrual.accrual_timestamp,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(accrual.accrual_id)
    }

    /// Get yield accrual history for account
    pub async fn get_yield_accrual_history(&self, account_id: Uuid, limit: Option<i64>) -> Result<Vec<YieldAccrualRecord>, AppError> {
        let mut query = "
            SELECT 
                accrual_id, account_id, accrual_period_start, accrual_period_end,
                opening_balance, yield_rate_applied, yield_amount_earned, accrual_timestamp
            FROM yield_accrual_records
            WHERE account_id = $1
            ORDER BY accrual_period_start DESC
        ".to_string();

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let accruals = sqlx::query_as!(
            YieldAccrualRecord,
            &query,
            account_id
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(accruals)
    }

    // ── Stellar AMM Pools ─────────────────────────────────────────────────────

    /// Create or update AMM pool
    pub async fn upsert_amm_pool(&self, pool: &StellarAmmPool) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO stellar_amm_pools (
                pool_id, asset_a_code, asset_a_issuer, asset_b_code, asset_b_issuer,
                total_pool_shares, asset_a_reserves, asset_b_reserves, current_price,
                trading_fee_bps, pool_status, tvl_24h_ago, volume_24h, fees_24h,
                last_updated_at, discovered_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (pool_id) DO UPDATE SET
                asset_a_reserves = EXCLUDED.asset_a_reserves,
                asset_b_reserves = EXCLUDED.asset_b_reserves,
                current_price = EXCLUDED.current_price,
                trading_fee_bps = EXCLUDED.trading_fee_bps,
                pool_status = EXCLUDED.pool_status,
                tvl_24h_ago = EXCLUDED.tvl_24h_ago,
                volume_24h = EXCLUDED.volume_24h,
                fees_24h = EXCLUDED.fees_24h,
                last_updated_at = EXCLUDED.last_updated_at
            "#,
            pool.pool_id,
            pool.asset_a_code,
            pool.asset_a_issuer,
            pool.asset_b_code,
            pool.asset_b_issuer,
            pool.total_pool_shares,
            pool.asset_a_reserves,
            pool.asset_b_reserves,
            pool.current_price,
            pool.trading_fee_bps,
            pool.pool_status as super::AmmPoolStatus,
            pool.tvl_24h_ago,
            pool.volume_24h,
            pool.fees_24h,
            pool.last_updated_at,
            pool.discovered_at,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    /// Get AMM pool
    pub async fn get_amm_pool(&self, pool_id: &str) -> Result<Option<StellarAmmPool>, AppError> {
        let pool = sqlx::query_as_opt!(
            StellarAmmPool,
            r#"
            SELECT 
                pool_id, asset_a_code, asset_a_issuer, asset_b_code, asset_b_issuer,
                total_pool_shares, asset_a_reserves, asset_b_reserves, current_price,
                trading_fee_bps, pool_status as "pool_status: super::AmmPoolStatus",
                tvl_24h_ago, volume_24h, fees_24h, last_updated_at, discovered_at
            FROM stellar_amm_pools
            WHERE pool_id = $1
            "#,
            pool_id
        )
        .await?;

        Ok(pool)
    }

    /// List AMM pools containing cNGN
    pub async fn list_cngn_amm_pools(&self) -> Result<Vec<StellarAmmPool>, AppError> {
        let pools = sqlx::query_as!(
            StellarAmmPool,
            r#"
            SELECT 
                pool_id, asset_a_code, asset_a_issuer, asset_b_code, asset_b_issuer,
                total_pool_shares, asset_a_reserves, asset_b_reserves, current_price,
                trading_fee_bps, pool_status as "pool_status: super::AmmPoolStatus",
                tvl_24h_ago, volume_24h, fees_24h, last_updated_at, discovered_at
            FROM stellar_amm_pools
            WHERE (asset_a_code = 'cNGN' OR asset_b_code = 'cNGN') AND pool_status = 'active'
            ORDER BY volume_24h DESC
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(pools)
    }

    // ── AMM Liquidity Positions ─────────────────────────────────────────────────

    /// Create AMM liquidity position
    pub async fn create_amm_position(&self, position: &AmmLiquidityPosition) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO amm_liquidity_positions (
                position_id, pool_id, strategy_id, shares_owned,
                asset_a_deposited, asset_b_deposited, initial_share_price,
                current_share_price, unrealized_yield, impermanent_loss,
                fee_income_earned, position_opened_at, last_valuation_at, position_status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING position_id
            "#,
            position.position_id,
            position.pool_id,
            position.strategy_id,
            position.shares_owned,
            position.asset_a_deposited,
            position.asset_b_deposited,
            position.initial_share_price,
            position.current_share_price,
            position.unrealized_yield,
            position.impermanent_loss,
            position.fee_income_earned,
            position.position_opened_at,
            position.last_valuation_at,
            position.position_status,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(position.position_id)
    }

    /// Get AMM positions by pool
    pub async fn get_amm_positions_by_pool(&self, pool_id: &str) -> Result<Vec<AmmLiquidityPosition>, AppError> {
        let positions = sqlx::query_as!(
            AmmLiquidityPosition,
            r#"
            SELECT 
                position_id, pool_id, strategy_id, shares_owned,
                asset_a_deposited, asset_b_deposited, initial_share_price,
                current_share_price, unrealized_yield, impermanent_loss,
                fee_income_earned, position_opened_at, last_valuation_at, position_status
            FROM amm_liquidity_positions
            WHERE pool_id = $1 AND position_status = 'active'
            "#,
            pool_id
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(positions)
    }

    // ── Circuit Breaker Trips ─────────────────────────────────────────────────

    /// Record circuit breaker trip
    pub async fn record_circuit_breaker_trip(&self, trip: &CircuitBreakerTrip) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO circuit_breaker_trips (
                trip_id, protocol_id, trigger, reason, tripped_at,
                resolved_at, resolved_by, resolution_reason
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING trip_id
            "#,
            trip.trip_id,
            trip.protocol_id,
            trip.trigger as super::CircuitBreakerTrigger,
            trip.reason,
            trip.tripped_at,
            trip.resolved_at,
            trip.resolved_by,
            trip.resolution_reason,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(trip.trip_id)
    }

    /// Get active circuit breaker trips
    pub async fn get_active_circuit_breaker_trips(&self) -> Result<Vec<CircuitBreakerTrip>, AppError> {
        let trips = sqlx::query_as!(
            CircuitBreakerTrip,
            r#"
            SELECT 
                trip_id, protocol_id, trigger as "trigger: super::CircuitBreakerTrigger", 
                reason, tripped_at, resolved_at, resolved_by, resolution_reason
            FROM circuit_breaker_trips
            WHERE resolved_at IS NULL
            ORDER BY tripped_at DESC
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(trips)
    }

    // ── Rebalancing Events ─────────────────────────────────────────────────────

    /// Create rebalancing event
    pub async fn create_rebalancing_event(&self, event: &RebalancingEvent) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO rebalancing_events (
                event_id, strategy_id, trigger_reason, pre_rebalancing_allocations,
                post_rebalancing_allocations, transaction_details, started_at,
                completed_at, status, error_message
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING event_id
            "#,
            event.event_id,
            event.strategy_id,
            event.trigger_reason,
            event.pre_rebalancing_allocations,
            event.post_rebalancing_allocations,
            event.transaction_details,
            event.started_at,
            event.completed_at,
            event.status,
            event.error_message,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(event.event_id)
    }

    // ── Yield Rate History ───────────────────────────────────────────────────

    /// Record yield rate
    pub async fn record_yield_rate(&self, history: &YieldRateHistory) -> Result<Uuid, AppError> {
        sqlx::query!(
            r#"
            INSERT INTO yield_rate_history (
                history_id, product_id, yield_rate, rate_source, recorded_at
            ) VALUES ($1, $2, $3, $4, $5)
            RETURNING history_id
            "#,
            history.history_id,
            history.product_id,
            history.yield_rate,
            history.rate_source,
            history.recorded_at,
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(history.history_id)
    }

    /// Get yield rate history
    pub async fn get_yield_rate_history(&self, product_id: Uuid, days: Option<i64>) -> Result<Vec<YieldRateHistory>, AppError> {
        let mut query = "
            SELECT 
                history_id, product_id, yield_rate, rate_source, recorded_at
            FROM yield_rate_history
            WHERE product_id = $1
        ".to_string();

        if let Some(days) = days {
            query.push_str(&format!(" AND recorded_at >= NOW() - INTERVAL '{}' DAY", days));
        }

        query.push_str(" ORDER BY recorded_at DESC");

        let history = sqlx::query_as!(
            YieldRateHistory,
            &query,
            product_id
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(history)
    }

    // ── Treasury and Analytics ─────────────────────────────────────────────────

    /// Get treasury exposure metrics
    pub async fn get_treasury_exposure_metrics(&self) -> Result<TreasuryExposureMetrics, AppError> {
        // This would typically involve complex queries across multiple tables
        // For now, return a placeholder implementation
        Ok(TreasuryExposureMetrics {
            total_treasury_value: BigDecimal::from(0),
            total_defi_exposure: BigDecimal::from(0),
            defi_exposure_percentage: 0.0,
            protocol_exposures: HashMap::new(),
            risk_metrics: super::TreasuryRiskMetrics {
                weighted_risk_score: 0.0,
                max_single_protocol_exposure_pct: 0.0,
                concentration_risk_score: 0.0,
                correlation_risk_score: 0.0,
                liquidity_risk_score: 0.0,
            },
            last_updated_at: Utc::now(),
        })
    }
}
