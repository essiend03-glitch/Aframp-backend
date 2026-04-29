use chrono::Utc;
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::database::DbPool;
use crate::error::AppError;
use super::{
    TreasuryAllocation, TreasuryExposureMetrics, TreasuryRiskMetrics, ProtocolExposure,
    TreasuryAllocationType, TreasuryAllocationStatus, DeFiConfig,
};

/// Treasury manager for overseeing platform fund allocations to DeFi protocols
pub struct TreasuryManager {
    db_pool: Arc<DbPool>,
    config: TreasuryConfig,
}

impl TreasuryManager {
    pub fn new(db_pool: Arc<DbPool>, config: TreasuryConfig) -> Self {
        Self { db_pool, config }
    }

    /// Allocate treasury funds to a DeFi protocol
    pub async fn allocate_to_protocol(
        &self,
        protocol_id: &str,
        amount: BigDecimal,
        allocation_type: TreasuryAllocationType,
        allocated_by: &str,
    ) -> Result<TreasuryAllocation, AppError> {
        // Validate allocation against treasury limits
        self.validate_allocation_limits(protocol_id, &amount).await?;

        let allocation = TreasuryAllocation {
            allocation_id: Uuid::new_v4(),
            protocol_id: protocol_id.to_string(),
            allocation_type,
            allocated_amount: amount.clone(),
            current_value: amount.clone(),
            yield_earned: BigDecimal::from(0),
            allocation_percentage: self.calculate_allocation_percentage(&amount).await?,
            allocated_at: Utc::now(),
            last_updated_at: Utc::now(),
            status: TreasuryAllocationStatus::Active,
        };

        // Record allocation in database
        sqlx::query!(
            r#"
            INSERT INTO treasury_allocations (
                allocation_id, protocol_id, allocation_type, allocated_amount,
                current_value, yield_earned, allocation_percentage, allocated_at,
                last_updated_at, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            allocation.allocation_id,
            allocation.protocol_id,
            allocation.allocation_type as super::TreasuryAllocationType,
            allocation.allocated_amount,
            allocation.current_value,
            allocation.yield_earned,
            allocation.allocation_percentage,
            allocation.allocated_at,
            allocation.last_updated_at,
            allocation.status as super::TreasuryAllocationStatus,
        )
        .execute(&*self.db_pool)
        .await?;

        tracing::info!(
            protocol_id = %protocol_id,
            amount = %amount,
            allocation_type = ?allocation_type,
            allocated_by = %allocated_by,
            "Treasury funds allocated to protocol"
        );

        Ok(allocation)
    }

    /// Get current treasury exposure metrics
    pub async fn get_exposure_metrics(&self) -> Result<TreasuryExposureMetrics, AppError> {
        // Get total treasury value
        let total_treasury_value = self.get_total_treasury_value().await?;
        
        // Get total DeFi exposure
        let total_defi_exposure = self.get_total_defi_exposure().await?;
        
        // Calculate exposure percentage
        let defi_exposure_percentage = if total_treasury_value > BigDecimal::from(0) {
            (&total_defi_exposure / &total_treasury_value).to_string().parse().unwrap_or(0.0) * 100.0
        } else {
            0.0
        };

        // Get protocol-specific exposures
        let protocol_exposures = self.get_protocol_exposures().await?;

        // Calculate risk metrics
        let risk_metrics = self.calculate_risk_metrics(&protocol_exposures, &total_defi_exposure).await?;

        Ok(TreasuryExposureMetrics {
            total_treasury_value,
            total_defi_exposure,
            defi_exposure_percentage,
            protocol_exposures,
            risk_metrics,
            last_updated_at: Utc::now(),
        })
    }

    /// Rebalance treasury allocations based on risk metrics
    pub async fn rebalance_treasury(&self, target_allocations: HashMap<String, f64>) -> Result<Vec<TreasuryAllocation>, AppError> {
        let current_metrics = self.get_exposure_metrics().await?;
        
        // Validate target allocations sum to 100%
        let total_target: f64 = target_allocations.values().sum();
        if (total_target - 100.0).abs() > 0.01 {
            return Err(AppError::BadRequest(format!(
                "Target allocations must sum to 100%%, got {:.2}%", total_target
            )));
        }

        // Check if rebalancing is needed
        let rebalancing_needed = self.is_rebalancing_needed(&current_metrics, &target_allocations).await?;
        if !rebalancing_needed {
            return Ok(Vec::new());
        }

        let mut new_allocations = Vec::new();

        for (protocol_id, target_percentage) in target_allocations {
            let target_amount = &current_metrics.total_treasury_value * (target_percentage / 100.0);
            
            // Create new allocation
            let allocation = self.allocate_to_protocol(
                &protocol_id,
                target_amount,
                TreasuryAllocationType::YieldStrategy,
                "treasury_rebalancing",
            ).await?;
            
            new_allocations.push(allocation);
        }

        tracing::info!(
            allocations_created = %new_allocations.len(),
            "Treasury rebalancing completed"
        );

        Ok(new_allocations)
    }

    /// Withdraw funds from a protocol
    pub async fn withdraw_from_protocol(
        &self,
        allocation_id: Uuid,
        amount: BigDecimal,
        reason: &str,
    ) -> Result<TreasuryAllocation, AppError> {
        // Get current allocation
        let allocation = self.get_treasury_allocation(allocation_id).await?;
        
        if allocation.status != TreasuryAllocationStatus::Active {
            return Err(AppError::BadRequest("Allocation is not active".to_string()));
        }

        if amount > allocation.current_value {
            return Err(AppError::BadRequest("Withdrawal amount exceeds current value".to_string()));
        }

        // Update allocation
        let new_current_value = &allocation.current_value - &amount;
        let new_status = if new_current_value == BigDecimal::from(0) {
            TreasuryAllocationStatus::Closed
        } else {
            TreasuryAllocationStatus::Active
        };

        sqlx::query!(
            r#"
            UPDATE treasury_allocations 
            SET current_value = $1, status = $2, last_updated_at = NOW()
            WHERE allocation_id = $3
            "#,
            new_current_value,
            new_status as super::TreasuryAllocationStatus,
            allocation_id,
        )
        .execute(&*self.db_pool)
        .await?;

        tracing::info!(
            allocation_id = %allocation_id,
            amount = %amount,
            reason = %reason,
            "Treasury funds withdrawn from protocol"
        );

        // Return updated allocation
        self.get_treasury_allocation(allocation_id).await
    }

    /// Get all treasury allocations
    pub async fn get_treasury_allocations(&self) -> Result<Vec<TreasuryAllocation>, AppError> {
        let allocations = sqlx::query_as!(
            TreasuryAllocation,
            r#"
            SELECT 
                allocation_id, protocol_id, allocation_type as "allocation_type: super::TreasuryAllocationType",
                allocated_amount, current_value, yield_earned, allocation_percentage,
                allocated_at, last_updated_at, status as "status: super::TreasuryAllocationStatus"
            FROM treasury_allocations
            ORDER BY allocated_at DESC
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(allocations)
    }

    /// Get treasury allocation by ID
    pub async fn get_treasury_allocation(&self, allocation_id: Uuid) -> Result<TreasuryAllocation, AppError> {
        let allocation = sqlx::query_as!(
            TreasuryAllocation,
            r#"
            SELECT 
                allocation_id, protocol_id, allocation_type as "allocation_type: super::TreasuryAllocationType",
                allocated_amount, current_value, yield_earned, allocation_percentage,
                allocated_at, last_updated_at, status as "status: super::TreasuryAllocationStatus"
            FROM treasury_allocations
            WHERE allocation_id = $1
            "#,
            allocation_id
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(allocation)
    }

    /// Update allocation value and yield
    pub async fn update_allocation_value(
        &self,
        allocation_id: Uuid,
        current_value: BigDecimal,
        yield_earned: BigDecimal,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE treasury_allocations 
            SET current_value = $1, yield_earned = $2, last_updated_at = NOW()
            WHERE allocation_id = $3
            "#,
            current_value,
            yield_earned,
            allocation_id,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    // ── Private Helper Methods ─────────────────────────────────────────────────

    /// Validate allocation against treasury limits
    async fn validate_allocation_limits(&self, protocol_id: &str, amount: &BigDecimal) -> Result<(), AppError> {
        let current_metrics = self.get_exposure_metrics().await?;
        
        // Check total DeFi exposure limit
        let new_total_exposure = &current_metrics.total_defi_exposure + amount;
        let max_defi_exposure = &current_metrics.total_treasury_value * (self.config.max_defi_exposure_pct / 100.0);
        
        if new_total_exposure > max_defi_exposure {
            return Err(AppError::BadRequest(format!(
                "Allocation would exceed maximum DeFi exposure limit of {}%",
                self.config.max_defi_exposure_pct
            )));
        }

        // Check single protocol exposure limit
        if let Some(protocol_exposure) = current_metrics.protocol_exposures.get(protocol_id) {
            let new_protocol_exposure = &protocol_exposure.current_value + amount;
            let max_protocol_exposure = &current_metrics.total_treasury_value * (self.config.max_single_protocol_exposure_pct / 100.0);
            
            if new_protocol_exposure > max_protocol_exposure {
                return Err(AppError::BadRequest(format!(
                    "Allocation would exceed maximum single protocol exposure limit of {}%",
                    self.config.max_single_protocol_exposure_pct
                )));
            }
        }

        Ok(())
    }

    /// Calculate allocation percentage of total treasury
    async fn calculate_allocation_percentage(&self, amount: &BigDecimal) -> Result<f64, AppError> {
        let total_treasury_value = self.get_total_treasury_value().await?;
        
        if total_treasury_value == BigDecimal::from(0) {
            return Ok(0.0);
        }

        let percentage = (amount / &total_treasury_value).to_string().parse().unwrap_or(0.0) * 100.0;
        Ok(percentage)
    }

    /// Get total treasury value
    async fn get_total_treasury_value(&self) -> Result<BigDecimal, AppError> {
        // In a real implementation, this would sum all platform assets
        // For now, use a placeholder value
        Ok(BigDecimal::from(100_000_000)) // $100M placeholder
    }

    /// Get total DeFi exposure
    async fn get_total_defi_exposure(&self) -> Result<BigDecimal, AppError> {
        let result = sqlx::query!(
            "SELECT COALESCE(SUM(current_value), 0) as total FROM treasury_allocations WHERE status = 'active'"
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(result.total.unwrap_or_else(|| BigDecimal::from(0)))
    }

    /// Get protocol-specific exposures
    async fn get_protocol_exposures(&self) -> Result<HashMap<String, ProtocolExposure>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                protocol_id, SUM(current_value) as current_value,
                SUM(yield_earned) as yield_earned, COUNT(*) as position_count
            FROM treasury_allocations
            WHERE status = 'active'
            GROUP BY protocol_id
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        let mut exposures = HashMap::new();

        for row in rows {
            let exposure = ProtocolExposure {
                protocol_id: row.protocol_id.clone(),
                protocol_name: row.protocol_id.clone(), // Would fetch from protocols table
                risk_tier: super::RiskTier::Tier1, // Would fetch from protocols table
                allocated_amount: row.current_value.clone(), // Simplified
                current_value: row.current_value.unwrap_or_else(|| BigDecimal::from(0)),
                exposure_percentage: 0.0, // Would calculate based on total treasury
                yield_earned: row.yield_earned.unwrap_or_else(|| BigDecimal::from(0)),
                position_count: row.position_count.unwrap_or(0) as u64,
            };
            
            exposures.insert(row.protocol_id, exposure);
        }

        Ok(exposures)
    }

    /// Calculate risk metrics
    async fn calculate_risk_metrics(
        &self,
        protocol_exposures: &HashMap<String, ProtocolExposure>,
        total_exposure: &BigDecimal,
    ) -> Result<TreasuryRiskMetrics, AppError> {
        let weighted_risk_score = self.calculate_weighted_risk_score(protocol_exposures, total_exposure).await?;
        let max_single_protocol_exposure_pct = self.calculate_max_protocol_exposure_pct(protocol_exposures, total_exposure).await?;
        let concentration_risk_score = self.calculate_concentration_risk_score(protocol_exposures).await?;
        let correlation_risk_score = self.calculate_correlation_risk_score(protocol_exposures).await?;
        let liquidity_risk_score = self.calculate_liquidity_risk_score(protocol_exposures).await?;

        Ok(TreasuryRiskMetrics {
            weighted_risk_score,
            max_single_protocol_exposure_pct,
            concentration_risk_score,
            correlation_risk_score,
            liquidity_risk_score,
        })
    }

    async fn calculate_weighted_risk_score(
        &self,
        protocol_exposures: &HashMap<String, ProtocolExposure>,
        total_exposure: &BigDecimal,
    ) -> Result<f64, AppError> {
        if *total_exposure == BigDecimal::from(0) {
            return Ok(0.0);
        }

        let mut weighted_sum = 0.0;

        for exposure in protocol_exposures.values() {
            let weight = (&exposure.current_value / total_exposure).to_string().parse().unwrap_or(0.0);
            let risk_score = match exposure.risk_tier {
                super::RiskTier::Tier1 => 0.1,
                super::RiskTier::Tier2 => 0.3,
                super::RiskTier::Tier3 => 0.7,
            };
            
            weighted_sum += weight * risk_score;
        }

        Ok(weighted_sum)
    }

    async fn calculate_max_protocol_exposure_pct(
        &self,
        protocol_exposures: &HashMap<String, ProtocolExposure>,
        total_exposure: &BigDecimal,
    ) -> Result<f64, AppError> {
        if *total_exposure == BigDecimal::from(0) {
            return Ok(0.0);
        }

        let max_exposure = protocol_exposures
            .values()
            .map(|e| (&e.current_value / total_exposure).to_string().parse().unwrap_or(0.0))
            .fold(0.0, f64::max);

        Ok(max_exposure * 100.0)
    }

    async fn calculate_concentration_risk_score(&self, protocol_exposures: &HashMap<String, ProtocolExposure>) -> Result<f64, AppError> {
        // Calculate Herfindahl-Hirschman Index (HHI) for concentration risk
        let total_exposure: BigDecimal = protocol_exposures
            .values()
            .map(|e| &e.current_value)
            .sum();

        if total_exposure == BigDecimal::from(0) {
            return Ok(0.0);
        }

        let hhi = protocol_exposures
            .values()
            .map(|e| {
                let market_share = (&e.current_value / &total_exposure).to_string().parse().unwrap_or(0.0);
                market_share * market_share
            })
            .sum();

        // Normalize HHI to 0-1 scale (max HHI is 1 for monopoly)
        Ok(hhi)
    }

    async fn calculate_correlation_risk_score(&self, _protocol_exposures: &HashMap<String, ProtocolExposure>) -> Result<f64, AppError> {
        // Simplified correlation risk calculation
        // In a real implementation, would use historical correlation data
        Ok(0.3) // Placeholder
    }

    async fn calculate_liquidity_risk_score(&self, _protocol_exposures: &HashMap<String, ProtocolExposure>) -> Result<f64, AppError> {
        // Simplified liquidity risk calculation
        // In a real implementation, would assess protocol liquidity depth
        Ok(0.2) // Placeholder
    }

    /// Check if rebalancing is needed
    async fn is_rebalancing_needed(
        &self,
        current_metrics: &TreasuryExposureMetrics,
        target_allocations: &HashMap<String, f64>,
    ) -> Result<bool, AppError> {
        let current_total = &current_metrics.total_treasury_value;

        for (protocol_id, target_percentage) in target_allocations {
            let target_amount = current_total * (*target_percentage / 100.0);
            
            if let Some(current_exposure) = current_metrics.protocol_exposures.get(protocol_id) {
                let diff = (&current_exposure.current_value - &target_amount).abs();
                let diff_percentage = (diff / current_total).to_string().parse().unwrap_or(0.0) * 100.0;
                
                if diff_percentage > self.config.rebalancing_threshold_pct {
                    return Ok(true);
                }
            } else if target_amount > BigDecimal::from(0) {
                // New allocation needed
                return Ok(true);
            }
        }

        Ok(false)
    }
}

/// Treasury configuration
#[derive(Debug, Clone)]
pub struct TreasuryConfig {
    pub max_defi_exposure_pct: f64,
    pub max_single_protocol_exposure_pct: f64,
    pub rebalancing_threshold_pct: f64,
    pub emergency_withdrawal_enabled: bool,
}

impl Default for TreasuryConfig {
    fn default() -> Self {
        Self {
            max_defi_exposure_pct: 30.0,
            max_single_protocol_exposure_pct: 10.0,
            rebalancing_threshold_pct: 5.0,
            emergency_withdrawal_enabled: true,
        }
    }
}
