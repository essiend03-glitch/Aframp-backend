use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use sqlx::types::BigDecimal;
use uuid::Uuid;

use crate::database::DbPool;
use crate::error::AppError;
use super::{
    DeFiProtocol, DeFiConfig, RiskController, GovernanceCommittee, TreasuryManager,
    YieldStrategy, StrategyAllocation, StrategyRiskParameters, GovernanceApprovalRecord,
    CngnSavingsProduct, CngnSavingsAccount, YieldAccrualRecord, WithdrawalRequest,
    StellarAmmPool, AmmLiquidityPosition, DeFiPosition, TreasuryExposureMetrics,
    CreateStrategyRequest, CreateSavingsAccountRequest, DepositRequest, WithdrawalRequest as WithdrawalReq,
    StrategyResponse, SavingsAccountResponse, DeFiOverviewResponse,
};

/// Main DeFi service orchestrating all DeFi operations
pub struct DeFiService {
    db_pool: Arc<DbPool>,
    config: DeFiConfig,
    risk_controller: Arc<RiskController>,
    governance_committee: Arc<GovernanceCommittee>,
    treasury_manager: Arc<TreasuryManager>,
    protocol_registry: Arc<ProtocolRegistry>,
}

impl DeFiService {
    pub fn new(
        db_pool: Arc<DbPool>,
        config: DeFiConfig,
        risk_controller: Arc<RiskController>,
        governance_committee: Arc<GovernanceCommittee>,
        treasury_manager: Arc<TreasuryManager>,
        protocol_registry: Arc<ProtocolRegistry>,
    ) -> Self {
        Self {
            db_pool,
            config,
            risk_controller,
            governance_committee,
            treasury_manager,
            protocol_registry,
        }
    }

    // ── Strategy Management ─────────────────────────────────────────────────────

    /// Create a new yield strategy
    pub async fn create_strategy(
        &self,
        request: CreateStrategyRequest,
        created_by: &str,
    ) -> Result<YieldStrategy, AppError> {
        let mut tx = self.db_pool.begin().await?;

        // Validate strategy configuration
        self.validate_strategy_request(&request)?;

        // Create strategy record
        let strategy = YieldStrategy {
            strategy_id: Uuid::new_v4(),
            strategy_name: request.strategy_name,
            description: request.description,
            strategy_type: request.strategy_type,
            target_yield_rate: request.target_yield_rate,
            min_acceptable_yield_rate: request.min_acceptable_yield_rate,
            max_acceptable_risk_score: request.max_acceptable_risk_score,
            total_allocated_amount: BigDecimal::from(0),
            max_allocation_limit: request.max_allocation_limit,
            rebalancing_frequency_secs: request.rebalancing_frequency_secs,
            rebalancing_triggers: request.rebalancing_triggers,
            strategy_status: super::StrategyStatus::Draft,
            governance_approval_record: GovernanceApprovalRecord {
                record_id: Uuid::new_v4(),
                strategy_id: Uuid::new_v4(), // Will be set after strategy creation
                submitted_by: created_by.to_string(),
                submitted_at: Utc::now(),
                required_approvals: self.config.min_governance_approvals,
                received_approvals: 0,
                approval_status: super::GovernanceStatus::Pending,
                approvals: Vec::new(),
                rejection_reason: None,
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Insert strategy
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
        .fetch_one(&mut *tx)
        .await?;

        // Create allocations
        for allocation_req in request.allocations {
            let allocation = StrategyAllocation {
                allocation_id: Uuid::new_v4(),
                strategy_id,
                protocol_id: allocation_req.protocol_id,
                target_allocation_percentage: allocation_req.target_allocation_percentage,
                current_allocation_amount: BigDecimal::from(0),
                min_allocation_percentage: allocation_req.min_allocation_percentage,
                max_allocation_percentage: allocation_req.max_allocation_percentage,
                last_rebalanced_at: Utc::now(),
                allocation_status: super::AllocationStatus::Active,
            };

            sqlx::query!(
                r#"
                INSERT INTO strategy_allocations (
                    allocation_id, strategy_id, protocol_id, target_allocation_percentage,
                    current_allocation_amount, min_allocation_percentage, max_allocation_percentage,
                    last_rebalanced_at, allocation_status
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
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
            .execute(&mut *tx)
            .await?;
        }

        // Create risk parameters
        let risk_params = StrategyRiskParameters {
            parameter_id: Uuid::new_v4(),
            strategy_id,
            max_single_protocol_exposure_pct: request.risk_parameters.max_single_protocol_exposure_pct,
            max_correlation_between_protocols: request.risk_parameters.max_correlation_between_protocols,
            max_acceptable_impermanent_loss_pct: request.risk_parameters.max_acceptable_impermanent_loss_pct,
            circuit_breaker_tvl_drop_threshold: request.risk_parameters.circuit_breaker_tvl_drop_threshold,
            emergency_withdrawal_trigger_conditions: request.risk_parameters.emergency_withdrawal_trigger_conditions,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        sqlx::query!(
            r#"
            INSERT INTO strategy_risk_parameters (
                parameter_id, strategy_id, max_single_protocol_exposure_pct,
                max_correlation_between_protocols, max_acceptable_impermanent_loss_pct,
                circuit_breaker_tvl_drop_threshold, emergency_withdrawal_trigger_conditions
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            risk_params.parameter_id,
            risk_params.strategy_id,
            risk_params.max_single_protocol_exposure_pct,
            risk_params.max_correlation_between_protocols,
            risk_params.max_acceptable_impermanent_loss_pct,
            risk_params.circuit_breaker_tvl_drop_threshold,
            risk_params.emergency_withdrawal_trigger_conditions,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Fetch complete strategy with allocations
        self.get_strategy(strategy_id).await
    }

    /// Submit strategy for governance approval
    pub async fn submit_strategy_for_approval(
        &self,
        strategy_id: Uuid,
        submitted_by: &str,
    ) -> Result<GovernanceApprovalRecord, AppError> {
        let strategy = self.get_strategy(strategy_id).await?;
        
        let approval_record = self.governance_committee
            .submit_strategy_for_approval(&strategy, submitted_by)
            .await?;

        // Store approval record in database
        sqlx::query!(
            r#"
            INSERT INTO strategy_governance_approvals (
                record_id, strategy_id, submitted_by, submitted_at,
                required_approvals, received_approvals, approval_status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (strategy_id) DO UPDATE SET
                submitted_by = EXCLUDED.submitted_by,
                submitted_at = EXCLUDED.submitted_at,
                required_approvals = EXCLUDED.required_approvals,
                received_approvals = EXCLUDED.received_approvals,
                approval_status = EXCLUDED.approval_status
            "#,
            approval_record.record_id,
            approval_record.strategy_id,
            approval_record.submitted_by,
            approval_record.submitted_at,
            approval_record.required_approvals,
            approval_record.received_approvals,
            approval_record.approval_status as super::GovernanceStatus,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(approval_record)
    }

    /// Activate an approved strategy
    pub async fn activate_strategy(
        &self,
        strategy_id: Uuid,
    ) -> Result<YieldStrategy, AppError> {
        // Check governance approval
        let approval_record = self.get_strategy_governance_approval(strategy_id).await?;
        if !self.governance_committee.can_activate_strategy(&approval_record).await? {
            return Err(AppError::BadRequest("Strategy does not have required governance approvals".to_string()));
        }

        // Update strategy status
        sqlx::query!(
            "UPDATE yield_strategies SET strategy_status = 'active', updated_at = NOW() WHERE strategy_id = $1",
            strategy_id
        )
        .execute(&*self.db_pool)
        .await?;

        // Deploy initial allocations
        self.deploy_strategy_allocations(strategy_id).await?;

        self.get_strategy(strategy_id).await
    }

    /// Get strategy by ID with all related data
    pub async fn get_strategy(&self, strategy_id: Uuid) -> Result<YieldStrategy, AppError> {
        let strategy = sqlx::query_as!(
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
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(strategy)
    }

    /// Get strategy with allocations and risk parameters
    pub async fn get_strategy_details(&self, strategy_id: Uuid) -> Result<StrategyResponse, AppError> {
        let strategy = self.get_strategy(strategy_id).await?;
        
        // Get allocations
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

        // Get risk parameters
        let risk_parameters = sqlx::query_as_opt!(
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

        // Get governance status
        let governance_status = self.get_strategy_governance_approval(strategy_id).await.ok();

        Ok(StrategyResponse {
            strategy,
            allocations,
            risk_parameters,
            performance: None,
            governance_status,
        })
    }

    // ── Savings Product Management ─────────────────────────────────────────────

    /// Create a new cNGN savings account
    pub async fn create_savings_account(
        &self,
        request: CreateSavingsAccountRequest,
        user_id: &str,
        ip_address: Option<&str>,
    ) -> Result<CngnSavingsAccount, AppError> {
        let mut tx = self.db_pool.begin().await?;

        // Validate product and user permissions
        let product = self.get_savings_product(request.product_id).await?;
        self.validate_savings_deposit_request(&request, &product).await?;

        // Create savings account
        let account = CngnSavingsAccount {
            account_id: Uuid::new_v4(),
            wallet_id: request.wallet_id,
            product_id: request.product_id,
            deposited_amount: request.deposit_amount.clone(),
            current_balance: request.deposit_amount.clone(),
            accrued_yield_to_date: BigDecimal::from(0),
            current_yield_rate: product.target_yield_rate,
            deposit_timestamp: Utc::now(),
            last_yield_accrual_timestamp: Utc::now(),
            withdrawal_eligibility_timestamp: Utc::now() + chrono::Duration::hours(product.lock_up_period_hours),
            account_status: super::SavingsAccountStatus::Active,
            risk_disclosure_accepted_at: Utc::now(),
            risk_disclosure_ip_address: ip_address.map(|s| s.to_string()),
        };

        // Insert account
        sqlx::query!(
            r#"
            INSERT INTO cngn_savings_accounts (
                account_id, wallet_id, product_id, deposited_amount, current_balance,
                accrued_yield_to_date, current_yield_rate, deposit_timestamp,
                last_yield_accrual_timestamp, withdrawal_eligibility_timestamp,
                account_status, risk_disclosure_accepted_at, risk_disclosure_ip_address
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
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
        .execute(&mut *tx)
        .await?;

        // Deploy funds to underlying strategy if configured
        if let Some(strategy_id) = product.underlying_strategy_id {
            self.deploy_savings_funds_to_strategy(account.account_id, strategy_id, &request.deposit_amount, &mut tx).await?;
        }

        tx.commit().await?;

        Ok(account)
    }

    /// Process deposit to savings account
    pub async fn deposit_to_savings_account(
        &self,
        request: DepositRequest,
    ) -> Result<CngnSavingsAccount, AppError> {
        let mut tx = self.db_pool.begin().await?;

        // Get account and validate
        let mut account = self.get_savings_account(request.account_id).await?;
        let product = self.get_savings_product(account.product_id).await?;
        
        self.validate_deposit_amount(&request.amount, &product).await?;

        // Update account balance
        account.deposited_amount += &request.amount;
        account.current_balance += &request.amount;

        sqlx::query!(
            r#"
            UPDATE cngn_savings_accounts 
            SET deposited_amount = $1, current_balance = $2, updated_at = NOW()
            WHERE account_id = $3
            "#,
            account.deposited_amount,
            account.current_balance,
            account.account_id,
        )
        .execute(&mut *tx)
        .await?;

        // Deploy additional funds to strategy
        if let Some(strategy_id) = product.underlying_strategy_id {
            self.deploy_savings_funds_to_strategy(account.account_id, strategy_id, &request.amount, &mut tx).await?;
        }

        tx.commit().await?;

        Ok(account)
    }

    /// Process withdrawal from savings account
    pub async fn withdraw_from_savings_account(
        &self,
        request: WithdrawalReq,
    ) -> Result<WithdrawalRequest, AppError> {
        let mut tx = self.db_pool.begin().await?;

        // Get account and validate
        let account = self.get_savings_account(request.account_id).await?;
        let product = self.get_savings_product(account.product_id).await?;
        
        self.validate_withdrawal_request(&request, &account, &product).await?;

        // Calculate penalty if early withdrawal
        let penalty_amount = self.calculate_early_withdrawal_penalty(&request, &account, &product)?;
        let net_amount = &request.amount - &penalty_amount;

        // Create withdrawal request
        let withdrawal = WithdrawalRequest {
            request_id: Uuid::new_v4(),
            account_id: request.account_id,
            requested_amount: request.amount,
            withdrawal_type: request.withdrawal_type,
            early_withdrawal_flag: self.is_early_withdrawal(&account, &product),
            penalty_amount: penalty_amount.clone(),
            net_withdrawal_amount: net_amount,
            request_timestamp: Utc::now(),
            settlement_timestamp: None,
            status: "pending".to_string(),
            transaction_hash: None,
        };

        // Insert withdrawal request
        sqlx::query!(
            r#"
            INSERT INTO withdrawal_requests (
                request_id, account_id, requested_amount, withdrawal_type,
                early_withdrawal_flag, penalty_amount, net_withdrawal_amount,
                request_timestamp, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            withdrawal.request_id,
            withdrawal.account_id,
            withdrawal.requested_amount,
            withdrawal.withdrawal_type as super::WithdrawalType,
            withdrawal.early_withdrawal_flag,
            withdrawal.penalty_amount,
            withdrawal.net_withdrawal_amount,
            withdrawal.request_timestamp,
            withdrawal.status,
        )
        .execute(&mut *tx)
        .await?;

        // Update account balance
        let new_balance = &account.current_balance - &request.amount;
        let new_status = if new_balance == BigDecimal::from(0) {
            super::SavingsAccountStatus::Closed
        } else {
            super::SavingsAccountStatus::Active
        };

        sqlx::query!(
            r#"
            UPDATE cngn_savings_accounts 
            SET current_balance = $1, account_status = $2, updated_at = NOW()
            WHERE account_id = $3
            "#,
            new_balance,
            new_status as super::SavingsAccountStatus,
            account.account_id,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(withdrawal)
    }

    // ── AMM Integration ────────────────────────────────────────────────────────

    /// Get all available Stellar AMM pools
    pub async fn get_amm_pools(&self) -> Result<Vec<StellarAmmPool>, AppError> {
        let pools = sqlx::query_as!(
            StellarAmmPool,
            r#"
            SELECT 
                pool_id, asset_a_code, asset_a_issuer, asset_b_code, asset_b_issuer,
                total_pool_shares, asset_a_reserves, asset_b_reserves, current_price,
                trading_fee_bps, pool_status as "pool_status: super::AmmPoolStatus",
                tvl_24h_ago, volume_24h, fees_24h, last_updated_at, discovered_at
            FROM stellar_amm_pools
            WHERE pool_status = 'active'
            ORDER BY volume_24h DESC
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(pools)
    }

    /// Get AMM pool by ID
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

    // ── Overview and Analytics ─────────────────────────────────────────────────

    /// Get comprehensive DeFi overview
    pub async fn get_defi_overview(&self) -> Result<DeFiOverviewResponse, AppError> {
        // Get treasury exposure metrics
        let treasury_metrics = self.treasury_manager.get_exposure_metrics().await?;

        // Get strategy breakdown
        let strategy_breakdown = self.get_strategy_performance_summaries().await?;

        // Get savings breakdown
        let savings_breakdown = self.get_savings_product_summaries().await?;

        Ok(DeFiOverviewResponse {
            total_exposure: treasury_metrics.total_defi_exposure,
            total_yield: treasury_metrics.total_yield_earned,
            active_strategies: strategy_breakdown.len() as u64,
            active_positions: 0, // TODO: Implement position counting
            average_yield_rate: 0.0, // TODO: Calculate weighted average
            risk_metrics: treasury_metrics.risk_metrics,
            protocol_breakdown: treasury_metrics.protocol_exposures.into_values().collect(),
            strategy_breakdown,
            savings_breakdown,
        })
    }

    // ── Helper Methods ────────────────────────────────────────────────────────

    async fn validate_strategy_request(&self, request: &CreateStrategyRequest) -> Result<(), AppError> {
        // Validate allocation percentages sum to 100%
        let total_allocation: f64 = request.allocations.iter()
            .map(|a| a.target_allocation_percentage)
            .sum();
        
        if (total_allocation - 100.0).abs() > 0.01 {
            return Err(AppError::BadRequest(format!(
                "Strategy allocation percentages must sum to 100%%, got {:.2}%", 
                total_allocation
            )));
        }

        // Validate each allocation
        for allocation in &request.allocations {
            // Check if protocol exists and is active
            let protocol = self.protocol_registry.get_protocol(&allocation.protocol_id)
                .ok_or_else(|| AppError::BadRequest(format!("Protocol not found: {}", allocation.protocol_id)))?;
            
            if !protocol.is_active() {
                return Err(AppError::BadRequest(format!("Protocol is not active: {}", allocation.protocol_id)));
            }

            // Validate allocation ranges
            if allocation.target_allocation_percentage < allocation.min_allocation_percentage ||
               allocation.target_allocation_percentage > allocation.max_allocation_percentage {
                return Err(AppError::BadRequest(
                    "Target allocation must be within min/max bounds".to_string()
                ));
            }
        }

        Ok(())
    }

    async fn deploy_strategy_allocations(&self, strategy_id: Uuid) -> Result<(), AppError> {
        // Get strategy allocations
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

        // Get strategy
        let strategy = self.get_strategy(strategy_id).await?;

        // Deploy to each protocol based on allocation percentages
        for allocation in allocations {
            let allocation_amount = &strategy.max_allocation_limit * 
                (allocation.target_allocation_percentage / 100.0);

            if allocation_amount > BigDecimal::from(0) {
                // Deploy funds to protocol
                self.deploy_to_protocol(
                    &allocation.protocol_id,
                    &allocation_amount,
                    strategy_id,
                ).await?;
            }
        }

        Ok(())
    }

    async fn deploy_to_protocol(
        &self,
        protocol_id: &str,
        amount: &BigDecimal,
        strategy_id: Uuid,
    ) -> Result<(), AppError> {
        let protocol = self.protocol_registry.get_protocol(protocol_id)
            .ok_or_else(|| AppError::BadRequest(format!("Protocol not found: {}", protocol_id)))?;

        // Validate risk controls
        let current_exposure = self.get_protocol_exposure(protocol_id).await?;
        let max_exposure = self.calculate_max_protocol_exposure(protocol_id).await?;
        
        let validation_result = self.risk_controller.validate_deposit(
            protocol.as_ref(),
            amount,
            &current_exposure,
            &max_exposure,
        ).await?;

        if !validation_result.passed {
            return Err(AppError::BadRequest(format!(
                "Risk validation failed: {}",
                validation_result.failed_validations
                    .iter()
                    .map(|v| &v.message)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        // Execute deposit
        let position = protocol.deposit(
            amount.clone(),
            "cNGN", // TODO: Make asset code configurable
            self.config.default_slippage_tolerance,
        ).await?;

        // Record position
        self.record_defi_position(position, strategy_id).await?;

        Ok(())
    }

    async fn record_defi_position(&self, position: DeFiPosition, strategy_id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO defi_positions (
                position_id, protocol_id, asset_code, deposited_amount, current_value,
                yield_earned, effective_yield_rate, position_opened_at, last_updated_at,
                position_status, protocol_position_id, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
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
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    async fn get_strategy_governance_approval(&self, strategy_id: Uuid) -> Result<GovernanceApprovalRecord, AppError> {
        let record = sqlx::query!(
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
        .fetch_one(&*self.db_pool)
        .await?;

        // Get individual approvals
        let approvals = sqlx::query_as!(
            super::GovernanceApproval,
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

        Ok(GovernanceApprovalRecord {
            record_id: record.record_id,
            strategy_id: record.strategy_id,
            submitted_by: record.submitted_by,
            submitted_at: record.submitted_at,
            required_approvals: record.required_approvals as usize,
            received_approvals: record.received_approvals as usize,
            approval_status: record.approval_status,
            approvals,
            rejection_reason: record.rejection_reason,
        })
    }

    async fn get_savings_product(&self, product_id: Uuid) -> Result<CngnSavingsProduct, AppError> {
        let product = sqlx::query_as!(
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
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(product)
    }

    async fn get_savings_account(&self, account_id: Uuid) -> Result<CngnSavingsAccount, AppError> {
        let account = sqlx::query_as!(
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
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(account)
    }

    // Additional helper methods would be implemented here...
    async fn validate_savings_deposit_request(&self, _request: &CreateSavingsAccountRequest, _product: &CngnSavingsProduct) -> Result<(), AppError> {
        // Implementation would validate deposit limits, KYC status, etc.
        Ok(())
    }

    async fn validate_deposit_amount(&self, _amount: &BigDecimal, _product: &CngnSavingsProduct) -> Result<(), AppError> {
        // Implementation would validate amount limits
        Ok(())
    }

    async fn validate_withdrawal_request(&self, _request: &WithdrawalReq, _account: &CngnSavingsAccount, _product: &CngnSavingsProduct) -> Result<(), AppError> {
        // Implementation would validate withdrawal eligibility, amounts, etc.
        Ok(())
    }

    fn calculate_early_withdrawal_penalty(&self, _request: &WithdrawalReq, _account: &CngnSavingsAccount, _product: &CngnSavingsProduct) -> Result<BigDecimal, AppError> {
        // Implementation would calculate penalty based on lock-up period
        Ok(BigDecimal::from(0))
    }

    fn is_early_withdrawal(&self, _account: &CngnSavingsAccount, _product: &CngnSavingsProduct) -> bool {
        // Implementation would check if withdrawal is before lock-up period
        false
    }

    async fn deploy_savings_funds_to_strategy(&self, _account_id: Uuid, _strategy_id: Uuid, _amount: &BigDecimal, _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), AppError> {
        // Implementation would deploy savings funds to the underlying strategy
        Ok(())
    }

    async fn get_protocol_exposure(&self, _protocol_id: &str) -> Result<BigDecimal, AppError> {
        // Implementation would get current exposure for a protocol
        Ok(BigDecimal::from(0))
    }

    async fn calculate_max_protocol_exposure(&self, _protocol_id: &str) -> Result<BigDecimal, AppError> {
        // Implementation would calculate maximum allowed exposure for a protocol
        Ok(BigDecimal::from(0))
    }

    async fn get_strategy_performance_summaries(&self) -> Result<Vec<super::StrategyPerformanceSummary>, AppError> {
        let strategies = sqlx::query!(
            r#"
            SELECT 
                s.strategy_id, s.strategy_name, s.target_yield_rate,
                s.total_allocated_amount, s.strategy_status,
                COALESCE(sp.effective_yield_rate, 0) as effective_yield_rate,
                COALESCE(sp.max_drawdown, 0) as max_drawdown
            FROM yield_strategies s
            LEFT JOIN (
                SELECT DISTINCT ON (strategy_id) 
                    strategy_id, effective_yield_rate, max_drawdown
                FROM strategy_performance 
                ORDER BY strategy_id, period_end DESC
            ) sp ON s.strategy_id = sp.strategy_id
            WHERE s.strategy_status = 'active'
            ORDER BY s.created_at DESC
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        let mut summaries = Vec::new();
        
        for strategy in strategies {
            let summary = super::StrategyPerformanceSummary {
                strategy_id: strategy.strategy_id,
                strategy_name: strategy.strategy_name,
                current_allocation: strategy.total_allocated_amount.unwrap_or_else(|| BigDecimal::from(0)),
                total_yield_earned: BigDecimal::from(0), // TODO: Calculate from performance records
                effective_yield_rate: strategy.effective_yield_rate.unwrap_or(0.0),
                max_drawdown: strategy.max_drawdown.unwrap_or(0.0),
                risk_score: 0.5, // TODO: Calculate from risk parameters
                sharpe_ratio: 0.0, // TODO: Calculate from performance data
                status: strategy.strategy_status,
                last_rebalanced: None, // TODO: Get from allocation records
            };
            
            summaries.push(summary);
        }

        Ok(summaries)
    }

    async fn get_savings_product_summaries(&self) -> Result<Vec<super::SavingsProductPerformanceSummary>, AppError> {
        let products = sqlx::query!(
            r#"
            SELECT 
                p.product_id, p.product_name, p.target_yield_rate, p.product_status,
                COUNT(a.account_id) as active_accounts,
                COALESCE(SUM(a.current_balance), 0) as total_deposits,
                COALESCE(SUM(a.accrued_yield_to_date), 0) as total_yield_accrued,
                AVG(a.current_yield_rate) as avg_yield_rate
            FROM cngn_savings_products p
            LEFT JOIN cngn_savings_accounts a ON p.product_id = a.product_id AND a.account_status = 'active'
            GROUP BY p.product_id, p.product_name, p.target_yield_rate, p.product_status
            ORDER BY p.created_at DESC
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        let mut summaries = Vec::new();
        
        for product in products {
            let summary = super::SavingsProductPerformanceSummary {
                product_id: product.product_id,
                product_name: product.product_name,
                active_accounts: product.active_accounts.unwrap_or(0) as u64,
                total_deposits: product.total_deposits.unwrap_or_else(|| BigDecimal::from(0)),
                total_yield_accrued: product.total_yield_accrued.unwrap_or_else(|| BigDecimal::from(0)),
                average_yield_rate: product.avg_yield_rate.unwrap_or(0.0),
                product_status: product.product_status,
            };
            
            summaries.push(summary);
        }

        Ok(summaries)
    }
}

/// Protocol registry for managing DeFi protocol implementations
pub struct ProtocolRegistry {
    protocols: HashMap<String, Arc<dyn DeFiProtocol>>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            protocols: HashMap::new(),
        }
    }

    pub fn register_protocol(&mut self, protocol: Arc<dyn DeFiProtocol>) {
        self.protocols.insert(protocol.protocol_id().to_string(), protocol);
    }

    pub fn get_protocol(&self, protocol_id: &str) -> Option<Arc<dyn DeFiProtocol>> {
        self.protocols.get(protocol_id).cloned()
    }

    pub fn list_protocols(&self) -> Vec<&str> {
        self.protocols.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
