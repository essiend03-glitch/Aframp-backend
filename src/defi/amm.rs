use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::chains::stellar::client::StellarClient;
use crate::database::DbPool;
use crate::error::AppError;
use super::{
    StellarAmmPool, AmmLiquidityPosition, AmmPositionSnapshot, AmmPoolStatus,
    CreateAmmPositionRequest, AmmSwapRequest, AmmPoolResponse, AmmPoolPerformanceSummary,
    AmmIncomeMetrics, PoolIncomeMetrics,
};

/// Stellar AMM Integration Service
pub struct AmmService {
    db_pool: Arc<DbPool>,
    stellar_client: Arc<StellarClient>,
    config: AmmConfig,
}

impl AmmService {
    pub fn new(db_pool: Arc<DbPool>, stellar_client: Arc<StellarClient>, config: AmmConfig) -> Self {
        Self {
            db_pool,
            stellar_client,
            config,
        }
    }

    /// Discover and cache all available Stellar AMM pools
    pub async fn discover_amm_pools(&self) -> Result<Vec<StellarAmmPool>, AppError> {
        // Fetch pools from Stellar Horizon API
        let horizon_pools = self.fetch_pools_from_horizon().await?;

        let mut discovered_pools = Vec::new();

        for horizon_pool in horizon_pools {
            // Filter for pools containing cNGN
            if self.pool_contains_cngn(&horizon_pool) {
                let pool = self.convert_horizon_pool_to_model(horizon_pool).await?;
                
                // Cache in database
                self.upsert_amm_pool(&pool).await?;
                
                discovered_pools.push(pool);
            }
        }

        tracing::info!(
            pools_discovered = discovered_pools.len(),
            "Stellar AMM pools discovered and cached"
        );

        Ok(discovered_pools)
    }

    /// Get all available AMM pools with optional platform positions
    pub async fn get_amm_pools(&self, include_positions: bool) -> Result<Vec<AmmPoolResponse>, AppError> {
        let pools = sqlx::query_as!(
            StellarAmmPool,
            r#"
            SELECT 
                pool_id, asset_a_code, asset_a_issuer, asset_b_code, asset_b_issuer,
                total_pool_shares, asset_a_reserves, asset_b_reserves, current_price,
                trading_fee_bps, pool_status as "pool_status: AmmPoolStatus",
                tvl_24h_ago, volume_24h, fees_24h, last_updated_at, discovered_at
            FROM stellar_amm_pools
            WHERE pool_status = 'active'
            ORDER BY volume_24h DESC
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        let mut responses = Vec::new();

        for pool in pools {
            let platform_position = if include_positions {
                self.get_platform_position_for_pool(&pool.pool_id).await?
            } else {
                None
            };

            let performance_metrics = self.calculate_pool_performance_metrics(&pool).await?;

            let response = AmmPoolResponse {
                pool,
                platform_position,
                performance_metrics: Some(performance_metrics),
            };

            responses.push(response);
        }

        Ok(responses)
    }

    /// Get specific AMM pool details
    pub async fn get_amm_pool(&self, pool_id: &str) -> Result<Option<AmmPoolResponse>, AppError> {
        let pool = sqlx::query_as_opt!(
            StellarAmmPool,
            r#"
            SELECT 
                pool_id, asset_a_code, asset_a_issuer, asset_b_code, asset_b_issuer,
                total_pool_shares, asset_a_reserves, asset_b_reserves, current_price,
                trading_fee_bps, pool_status as "pool_status: AmmPoolStatus",
                tvl_24h_ago, volume_24h, fees_24h, last_updated_at, discovered_at
            FROM stellar_amm_pools
            WHERE pool_id = $1
            "#,
            pool_id
        )
        .await?;

        if let Some(pool) = pool {
            let platform_position = self.get_platform_position_for_pool(&pool.pool_id).await?;
            let performance_metrics = self.calculate_pool_performance_metrics(&pool).await?;

            Ok(Some(AmmPoolResponse {
                pool,
                platform_position,
                performance_metrics: Some(performance_metrics),
            }))
        } else {
            Ok(None)
        }
    }

    /// Create a new AMM liquidity position
    pub async fn create_amm_position(
        &self,
        request: CreateAmmPositionRequest,
        user_id: &str,
    ) -> Result<AmmLiquidityPosition, AppError> {
        let mut tx = self.db_pool.begin().await?;

        // Validate pool exists and is active
        let pool = self.get_amm_pool(&request.pool_id).await?
            .ok_or_else(|| AppError::BadRequest("Pool not found".to_string()))?;

        if pool.pool_status != AmmPoolStatus::Active {
            return Err(AppError::BadRequest("Pool is not active".to_string()));
        }

        // Calculate optimal deposit amounts
        let optimal_amounts = self.calculate_optimal_deposit_amounts(&request.pool_id, &request.asset_a_amount).await?;

        // Validate slippage tolerance
        let slippage = self.calculate_deposit_slippage(&optimal_amounts).await?;
        if slippage > request.slippage_tolerance {
            return Err(AppError::BadRequest(format!(
                "Slippage {} exceeds tolerance {}", slippage, request.slippage_tolerance
            )));
        }

        // Execute liquidity deposit transaction on Stellar
        let transaction_result = self.execute_liquidity_deposit_transaction(
            &request.pool_id,
            &optimal_amounts.asset_a_amount,
            &optimal_amounts.asset_b_amount,
        ).await?;

        // Create position record
        let position = AmmLiquidityPosition {
            position_id: Uuid::new_v4(),
            pool_id: request.pool_id,
            strategy_id: None, // Direct user position
            shares_owned: transaction_result.shares_received,
            asset_a_deposited: optimal_amounts.asset_a_amount,
            asset_b_deposited: optimal_amounts.asset_b_amount,
            initial_share_price: optimal_amounts.pool_share_price,
            current_share_price: optimal_amounts.pool_share_price,
            unrealized_yield: BigDecimal::from(0),
            impermanent_loss: BigDecimal::from(0),
            fee_income_earned: BigDecimal::from(0),
            position_opened_at: Utc::now(),
            last_valuation_at: Utc::now(),
            position_status: "active".to_string(),
        };

        // Store position
        sqlx::query!(
            r#"
            INSERT INTO amm_liquidity_positions (
                position_id, pool_id, strategy_id, shares_owned,
                asset_a_deposited, asset_b_deposited, initial_share_price,
                current_share_price, unrealized_yield, impermanent_loss,
                fee_income_earned, position_opened_at, last_valuation_at, position_status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
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
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            position_id = %position.position_id,
            pool_id = %position.pool_id,
            user_id = %user_id,
            shares = %position.shares_owned,
            "AMM liquidity position created"
        );

        Ok(position)
    }

    /// Execute swap through AMM pool
    pub async fn execute_amm_swap(
        &self,
        request: AmmSwapRequest,
        user_id: &str,
    ) -> Result<super::DeFiSwapResult, AppError> {
        // Validate pool and assets
        let pool = self.get_amm_pool(&request.pool_id).await?
            .ok_or_else(|| AppError::BadRequest("Pool not found".to_string()))?;

        if !self.pool_supports_swap(&pool, &request.from_asset, &request.to_asset) {
            return Err(AppError::BadRequest("Pool does not support this swap".to_string()));
        }

        // Calculate expected output and slippage
        let swap_calculation = self.calculate_swap_output(&request).await?;
        
        if swap_calculation.slippage_pct > request.slippage_tolerance {
            return Err(AppError::BadRequest(format!(
                "Slippage {} exceeds tolerance {}", swap_calculation.slippage_pct, request.slippage_tolerance
            )));
        }

        // Execute swap transaction on Stellar
        let transaction_result = self.execute_swap_transaction(&request, &swap_calculation).await?;

        tracing::info!(
            pool_id = %request.pool_id,
            from_asset = %request.from_asset,
            to_asset = %request.to_asset,
            input_amount = %request.amount,
            output_amount = %swap_calculation.output_amount,
            user_id = %user_id,
            "AMM swap executed"
        );

        Ok(super::DeFiSwapResult {
            from_asset: request.from_asset,
            to_asset: request.to_asset,
            input_amount: request.amount,
            output_amount: swap_calculation.output_amount,
            slippage_pct: swap_calculation.slippage_pct,
            fees_paid: swap_calculation.fees_paid,
            transaction_hash: transaction_result.transaction_hash,
            completed_at: Utc::now(),
        })
    }

    /// Background job: Update all AMM pool data and position values
    pub async fn update_all_pool_data(&self) -> Result<u64, AppError> {
        let pools = self.list_active_pools().await?;
        let mut updated_count = 0;

        for pool in pools {
            if let Ok(()) = self.update_pool_data(&pool.pool_id).await {
                updated_count += 1;
            }
        }

        // Update position values for all active positions
        if let Ok(positions) = self.get_all_active_positions().await {
            for position in positions {
                if let Ok(()) = self.update_position_valuation(&position.position_id).await {
                    // Position updated successfully
                }
            }
        }

        tracing::info!(
            pools_updated = updated_count,
            total_pools = pools.len(),
            "AMM pool data update job completed"
        );

        Ok(updated_count)
    }

    /// Background job: Track impermanent loss and alert on thresholds
    pub async fn track_impermanent_loss(&self) -> Result<Vec<Uuid>, AppError> {
        let positions = self.get_all_active_positions().await?;
        let mut alert_positions = Vec::new();

        for position in positions {
            let current_il = self.calculate_current_impermanent_loss(&position).await?;
            
            if current_il > self.config.impermanent_loss_alert_threshold {
                alert_positions.push(position.position_id);
                
                // Send alert (would integrate with alerting system)
                tracing::warn!(
                    position_id = %position.position_id,
                    pool_id = %position.pool_id,
                    impermanent_loss_pct = %current_il,
                    "Impermanent loss threshold exceeded"
                );
            }
        }

        Ok(alert_positions)
    }

    /// Get AMM income metrics for reporting
    pub async fn get_amm_income_metrics(
        &self,
        period_start: Option<DateTime<Utc>>,
        period_end: Option<DateTime<Utc>>,
    ) -> Result<AmmIncomeMetrics, AppError> {
        let end = period_end.unwrap_or_else(Utc::now);
        let start = period_start.unwrap_or_else(|| end - chrono::Duration::days(30));

        let pool_metrics = sqlx::query!(
            r#"
            SELECT 
                pool_id, asset_a_code, asset_b_code,
                SUM(fee_income_earned) as total_fee_income,
                COUNT(*) as position_count
            FROM amm_liquidity_positions p
            JOIN stellar_amm_pools pool ON p.pool_id = pool.pool_id
            WHERE p.position_status = 'active'
            GROUP BY pool_id, asset_a_code, asset_b_code
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        let mut pool_breakdown = Vec::new();
        let mut total_fee_income = BigDecimal::from(0);

        for pool in pool_metrics {
            let fee_income = pool.total_fee_income.unwrap_or_else(|| BigDecimal::from(0));
            let volume = self.estimate_pool_volume(&pool.pool_id, &start, &end).await?;
            let effective_apr = self.calculate_effective_apr(&fee_income, &volume).await?;

            pool_breakdown.push(PoolIncomeMetrics {
                pool_id: pool.pool_id,
                asset_pair: format!("{}/{}", pool.asset_a_code, pool.asset_b_code),
                fee_income: fee_income.clone(),
                volume,
                effective_apr,
            });

            total_fee_income += &fee_income;
        }

        Ok(AmmIncomeMetrics {
            period_start: start,
            period_end: end,
            total_fee_income,
            pool_breakdown,
        })
    }

    // ── Helper Methods ────────────────────────────────────────────────────────

    async fn fetch_pools_from_horizon(&self) -> Result<Vec<HorizonPool>, AppError> {
        // Implementation would call Stellar Horizon API to fetch all AMM pools
        // For now, return placeholder data
        Ok(vec![
            HorizonPool {
                id: "pool_cngn_xlm".to_string(),
                asset_a: Asset { code: "cNGN".to_string(), issuer: None },
                asset_b: Asset { code: "XLM".to_string(), issuer: None },
                total_shares: BigDecimal::from(1000000),
                asset_a_reserves: BigDecimal::from(500000000),
                asset_b_reserves: BigDecimal::from(100000000),
                fee_bps: 30,
            },
            HorizonPool {
                id: "pool_cngn_usdc".to_string(),
                asset_a: Asset { code: "cNGN".to_string(), issuer: None },
                asset_b: Asset { code: "USDC".to_string(), issuer: Some("GBBD47IF2LQW3XHO2HIYEDKGEZVUO4JZKU3FJOQQXK5I2RVEYPNHD".to_string()) },
                total_shares: BigDecimal::from(500000),
                asset_a_reserves: BigDecimal::from(250000000),
                asset_b_reserves: BigDecimal::from(50000000),
                fee_bps: 30,
            },
        ])
    }

    fn pool_contains_cngn(&self, pool: &HorizonPool) -> bool {
        pool.asset_a.code == "cNGN" || pool.asset_b.code == "cNGN"
    }

    async fn convert_horizon_pool_to_model(&self, horizon_pool: HorizonPool) -> Result<StellarAmmPool, AppError> {
        let current_price = &horizon_pool.asset_b_reserves / &horizon_pool.asset_a_reserves;
        let total_value = &horizon_pool.asset_a_reserves + &horizon_pool.asset_b_reserves;

        Ok(StellarAmmPool {
            pool_id: horizon_pool.id,
            asset_a_code: horizon_pool.asset_a.code,
            asset_a_issuer: horizon_pool.asset_a.issuer,
            asset_b_code: horizon_pool.asset_b.code,
            asset_b_issuer: horizon_pool.asset_b.issuer,
            total_pool_shares: horizon_pool.total_shares,
            asset_a_reserves: horizon_pool.asset_a_reserves,
            asset_b_reserves: horizon_pool.asset_b_reserves,
            current_price,
            trading_fee_bps: horizon_pool.fee_bps,
            pool_status: AmmPoolStatus::Active,
            tvl_24h_ago: None, // Would fetch historical data
            volume_24h: BigDecimal::from(1000000), // Placeholder
            fees_24h: BigDecimal::from(3000), // Placeholder
            last_updated_at: Utc::now(),
            discovered_at: Utc::now(),
        })
    }

    async fn upsert_amm_pool(&self, pool: &StellarAmmPool) -> Result<(), AppError> {
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
            pool.pool_status as AmmPoolStatus,
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

    async fn get_platform_position_for_pool(&self, pool_id: &str) -> Result<Option<AmmLiquidityPosition>, AppError> {
        let position = sqlx::query_as_opt!(
            AmmLiquidityPosition,
            r#"
            SELECT 
                position_id, pool_id, strategy_id, shares_owned,
                asset_a_deposited, asset_b_deposited, initial_share_price,
                current_share_price, unrealized_yield, impermanent_loss,
                fee_income_earned, position_opened_at, last_valuation_at, position_status
            FROM amm_liquidity_positions
            WHERE pool_id = $1 AND position_status = 'active'
            LIMIT 1
            "#,
            pool_id
        )
        .await?;

        Ok(position)
    }

    async fn calculate_pool_performance_metrics(&self, pool: &StellarAmmPool) -> Result<AmmPoolPerformanceSummary, AppError> {
        let total_liquidity = &pool.asset_a_reserves + &pool.asset_b_reserves;
        let apr = self.calculate_pool_apr(pool).await?;
        let impermanent_loss_24h = self.calculate_impermanent_loss_24h(pool).await?;
        
        let active_positions = sqlx::query!(
            "SELECT COUNT(*) as count FROM amm_liquidity_positions WHERE pool_id = $1 AND position_status = 'active'",
            pool.pool_id
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(AmmPoolPerformanceSummary {
            pool_id: pool.pool_id.clone(),
            asset_pair: format!("{}/{}", pool.asset_a_code, pool.asset_b_code),
            total_liquidity,
            volume_24h: pool.volume_24h.clone(),
            fees_24h: pool.fees_24h.clone(),
            apr,
            impermanent_loss_24h,
            active_positions: active_positions.count.unwrap_or(0) as u64,
            pool_status: format!("{:?}", pool.pool_status),
        })
    }

    async fn calculate_optimal_deposit_amounts(&self, pool_id: &str, asset_a_amount: &BigDecimal) -> Result<OptimalDepositAmounts, AppError> {
        let pool = self.get_amm_pool(pool_id).await?
            .ok_or_else(|| AppError::BadRequest("Pool not found".to_string()))?;

        let asset_b_amount = asset_a_amount * &pool.asset_b_reserves / &pool.asset_a_reserves;
        let share_price = (&pool.asset_a_reserves + &pool.asset_b_reserves) / &pool.total_pool_shares;

        Ok(OptimalDepositAmounts {
            asset_a_amount: asset_a_amount.clone(),
            asset_b_amount,
            pool_share_price: share_price,
        })
    }

    async fn calculate_deposit_slippage(&self, amounts: &OptimalDepositAmounts) -> Result<f64, AppError> {
        // Simplified slippage calculation for AMM deposits
        // In a real implementation, would be more complex
        Ok(0.005) // 0.5% placeholder
    }

    async fn execute_liquidity_deposit_transaction(
        &self,
        _pool_id: &str,
        _asset_a_amount: &BigDecimal,
        _asset_b_amount: &BigDecimal,
    ) -> Result<TransactionResult, AppError> {
        // Implementation would build and submit Stellar transaction
        // For now, return placeholder
        Ok(TransactionResult {
            transaction_hash: format!("stellar_tx_{}", Uuid::new_v4()),
            shares_received: BigDecimal::from(1000),
            gas_used: 0,
        })
    }

    fn pool_supports_swap(&self, pool: &StellarAmmPool, from_asset: &str, to_asset: &str) -> bool {
        (pool.asset_a_code == from_asset && pool.asset_b_code == to_asset) ||
        (pool.asset_a_code == to_asset && pool.asset_b_code == from_asset)
    }

    async fn calculate_swap_output(&self, request: &AmmSwapRequest) -> Result<SwapCalculation, AppError> {
        let pool = self.get_amm_pool(&request.pool_id).await?
            .ok_or_else(|| AppError::BadRequest("Pool not found".to_string()))?;

        let (reserve_in, reserve_out) = if pool.asset_a_code == request.from_asset {
            (&pool.asset_a_reserves, &pool.asset_b_reserves)
        } else {
            (&pool.asset_b_reserves, &pool.asset_a_reserves)
        };

        // Constant product formula: x * y = k
        let input_amount_with_fee = &request.amount * (1.0 - (pool.trading_fee_bps as f64 / 10000.0));
        let numerator = input_amount_with_fee * reserve_out;
        let denominator = reserve_in + &input_amount_with_fee;
        let output_amount = numerator / denominator;

        let slippage_pct = ((&request.amount - &output_amount) / &request.amount).to_string().parse().unwrap_or(0.0);
        let fees_paid = &request.amount * (pool.trading_fee_bps as f64 / 10000.0);

        Ok(SwapCalculation {
            output_amount,
            slippage_pct,
            fees_paid,
        })
    }

    async fn execute_swap_transaction(
        &self,
        _request: &AmmSwapRequest,
        _calculation: &SwapCalculation,
    ) -> Result<TransactionResult, AppError> {
        // Implementation would build and submit Stellar path payment transaction
        Ok(TransactionResult {
            transaction_hash: format!("stellar_swap_{}", Uuid::new_v4()),
            shares_received: BigDecimal::from(0), // Not applicable for swaps
            gas_used: 0,
        })
    }

    // Additional helper methods would be implemented here...
    async fn list_active_pools(&self) -> Result<Vec<StellarAmmPool>, AppError> {
        sqlx::query_as!(
            StellarAmmPool,
            r#"
            SELECT 
                pool_id, asset_a_code, asset_a_issuer, asset_b_code, asset_b_issuer,
                total_pool_shares, asset_a_reserves, asset_b_reserves, current_price,
                trading_fee_bps, pool_status as "pool_status: AmmPoolStatus",
                tvl_24h_ago, volume_24h, fees_24h, last_updated_at, discovered_at
            FROM stellar_amm_pools
            WHERE pool_status = 'active'
            "#
        )
        .fetch_all(&*self.db_pool)
        .await
    }

    async fn update_pool_data(&self, _pool_id: &str) -> Result<(), AppError> {
        // Implementation would fetch fresh data from Horizon and update database
        Ok(())
    }

    async fn get_all_active_positions(&self) -> Result<Vec<AmmLiquidityPosition>, AppError> {
        sqlx::query_as!(
            AmmLiquidityPosition,
            r#"
            SELECT 
                position_id, pool_id, strategy_id, shares_owned,
                asset_a_deposited, asset_b_deposited, initial_share_price,
                current_share_price, unrealized_yield, impermanent_loss,
                fee_income_earned, position_opened_at, last_valuation_at, position_status
            FROM amm_liquidity_positions
            WHERE position_status = 'active'
            "#
        )
        .fetch_all(&*self.db_pool)
        .await
    }

    async fn update_position_valuation(&self, _position_id: Uuid) -> Result<(), AppError> {
        // Implementation would recalculate position value based on current pool state
        Ok(())
    }

    async fn calculate_current_impermanent_loss(&self, _position: &AmmLiquidityPosition) -> Result<f64, AppError> {
        // Implementation would calculate current impermanent loss
        Ok(0.0) // Placeholder
    }

    async fn calculate_pool_apr(&self, _pool: &StellarAmmPool) -> Result<f64, AppError> {
        // Implementation would calculate APR based on fees and TVL
        Ok(0.08) // 8% placeholder
    }

    async fn calculate_impermanent_loss_24h(&self, _pool: &StellarAmmPool) -> Result<f64, AppError> {
        // Implementation would calculate 24h impermanent loss
        Ok(0.0) // Placeholder
    }

    async fn estimate_pool_volume(&self, _pool_id: &str, _start: &DateTime<Utc>, _end: &DateTime<Utc>) -> Result<BigDecimal, AppError> {
        // Implementation would estimate volume from transaction data
        Ok(BigDecimal::from(1000000)) // Placeholder
    }

    async fn calculate_effective_apr(&self, fee_income: &BigDecimal, volume: &BigDecimal) -> Result<f64, AppError> {
        if *volume == BigDecimal::from(0) {
            return Ok(0.0);
        }
        
        let fee_rate = (fee_income / volume).to_string().parse().unwrap_or(0.0);
        Ok(fee_rate * 365.0) // Annualized
    }
}

// ── Supporting Types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HorizonPool {
    pub id: String,
    pub asset_a: Asset,
    pub asset_b: Asset,
    pub total_shares: BigDecimal,
    pub asset_a_reserves: BigDecimal,
    pub asset_b_reserves: BigDecimal,
    pub fee_bps: i32,
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub code: String,
    pub issuer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OptimalDepositAmounts {
    pub asset_a_amount: BigDecimal,
    pub asset_b_amount: BigDecimal,
    pub pool_share_price: BigDecimal,
}

#[derive(Debug, Clone)]
pub struct SwapCalculation {
    pub output_amount: BigDecimal,
    pub slippage_pct: f64,
    pub fees_paid: BigDecimal,
}

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub transaction_hash: String,
    pub shares_received: BigDecimal,
    pub gas_used: u64,
}

/// AMM configuration
#[derive(Debug, Clone)]
pub struct AmmConfig {
    pub max_slippage_tolerance: f64,
    pub min_liquidity_amount: BigDecimal,
    pub impermanent_loss_alert_threshold: f64,
    pub position_update_interval_secs: u64,
}

impl Default for AmmConfig {
    fn default() -> Self {
        Self {
            max_slippage_tolerance: 0.01,
            min_liquidity_amount: BigDecimal::from(1000),
            impermanent_loss_alert_threshold: 0.10, // 10%
            position_update_interval_secs: 300, // 5 minutes
        }
    }
}
