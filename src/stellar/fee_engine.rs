/// Dynamic fee adjustment engine for Stellar surge pricing
/// 
/// Queries Horizon's /fee_stats endpoint to determine network congestion levels
/// and dynamically adjusts transaction submission fees to guarantee inclusion
/// within the immediate next ledger.

use crate::stellar::error::{SubmissionError, SubmissionResult};
use crate::stellar::models::{FeeConfiguration, FeeStats};
use chrono::{DateTime, Utc, Duration};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Dynamic fee engine with caching and surge pricing
pub struct DynamicFeeEngine {
    config: FeeConfiguration,
    cache: Arc<RwLock<FeeCache>>,
    horizon_client: reqwest::Client,
    horizon_url: String,
}

struct FeeCache {
    last_stats: Option<(FeeStats, DateTime<Utc>)>,
    cache_ttl_seconds: i64,
}

impl DynamicFeeEngine {
    /// Create a new dynamic fee engine
    pub fn new(
        config: FeeConfiguration,
        horizon_url: String,
    ) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(FeeCache {
                last_stats: None,
                cache_ttl_seconds: 10,
            })),
            horizon_client: reqwest::Client::new(),
            horizon_url,
        }
    }

    /// Query current fee stats from Horizon with caching
    pub async fn get_fee_stats(&self) -> SubmissionResult<FeeStats> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some((stats, cached_at)) = &cache.last_stats {
                let age = Utc::now().signed_duration_since(*cached_at);
                if age.num_seconds() < cache.cache_ttl_seconds {
                    return Ok(stats.clone());
                }
            }
        }

        // Fetch from Horizon
        let url = format!("{}/fee_stats", self.horizon_url);
        let response = self
            .horizon_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| SubmissionError::HorizonApi(format!("fee_stats request failed: {}", e)))?;

        let stats: FeeStats = response
            .json()
            .await
            .map_err(|e| {
                SubmissionError::HorizonApi(format!("failed to parse fee_stats response: {}", e))
            })?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.last_stats = Some((stats.clone(), Utc::now()));
        }

        Ok(stats)
    }

    /// Calculate optimal submission fee based on current network conditions
    pub async fn calculate_fee(&self, operation_count: i32) -> SubmissionResult<i64> {
        let fee_stats = self.get_fee_stats().await?;
        let base_fee = fee_stats.last_ledger_base_fee;

        // Check network capacity
        let capacity_usage: f64 = fee_stats
            .network_capacity_usage
            .parse()
            .unwrap_or(0.5);

        let per_op_fee = if capacity_usage > self.config.surge_threshold {
            // Network is congested - use surge pricing
            let surge_fee = (base_fee as f64 * self.config.surge_multiplier) as i64;
            surge_fee.min(self.config.max_fee).max(self.config.min_fee)
        } else {
            // Normal capacity - use base fee
            base_fee.min(self.config.max_fee).max(self.config.min_fee)
        };

        // Total fee = per-operation fee × operation count
        let total_fee = per_op_fee * operation_count as i64;
        Ok(total_fee.min(self.config.max_fee).max(self.config.min_fee))
    }

    /// Calculate fee with explicit surge multiplier (for testing)
    pub fn calculate_fee_with_multiplier(
        &self,
        operation_count: i32,
        base_fee: i64,
        surge_multiplier: f64,
    ) -> i64 {
        let per_op_fee = (base_fee as f64 * surge_multiplier) as i64;
        let per_op_fee = per_op_fee.min(self.config.max_fee).max(self.config.min_fee);
        let total_fee = per_op_fee * operation_count as i64;
        total_fee.min(self.config.max_fee).max(self.config.min_fee)
    }

    /// Get the surge fee percentage (100 = no surge, 150 = 50% surge)
    pub async fn get_surge_percent(&self) -> SubmissionResult<Decimal> {
        let fee_stats = self.get_fee_stats().await?;
        let capacity_usage: f64 = fee_stats
            .network_capacity_usage
            .parse()
            .unwrap_or(0.5);

        let multiplier = if capacity_usage > self.config.surge_threshold {
            self.config.surge_multiplier
        } else {
            1.0
        };

        let percent = (multiplier * 100.0) as i64;
        Ok(sqlx::types::Decimal::from(percent))
    }

    /// Get current capacity usage percentage
    pub async fn get_capacity_usage(&self) -> SubmissionResult<f64> {
        let fee_stats = self.get_fee_stats().await?;
        fee_stats
            .network_capacity_usage
            .parse()
            .map_err(|_| SubmissionError::FeeCalculationError("invalid capacity usage".to_string()))
    }

    /// Clear the fee cache (for testing)
    #[cfg(test)]
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.last_stats = None;
    }

    /// Get cached fee stats if available
    pub async fn get_cached_stats(&self) -> SubmissionResult<Option<FeeStats>> {
        let cache = self.cache.read().await;
        Ok(cache.last_stats.as_ref().map(|(stats, _)| stats.clone()))
    }
}

// Import Decimal type for use in methods
use sqlx::types::Decimal;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_engine() -> DynamicFeeEngine {
        let config = FeeConfiguration {
            base_fee: 100,
            min_fee: 100,
            max_fee: 10_000,
            surge_threshold: 0.8,
            surge_multiplier: 1.5,
            low_capacity_fee: 1_000,
        };
        DynamicFeeEngine::new(config, "https://horizon-testnet.stellar.org".to_string())
    }

    #[test]
    fn test_calculate_fee_with_multiplier_no_surge() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(1, 100, 1.0);
        assert_eq!(fee, 100);
    }

    #[test]
    fn test_calculate_fee_with_multiplier_surge() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(1, 100, 1.5);
        assert_eq!(fee, 150);
    }

    #[test]
    fn test_calculate_fee_respects_max() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(100, 100, 2.0);
        assert_eq!(fee, 10_000); // Capped at max_fee
    }

    #[test]
    fn test_calculate_fee_respects_min() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(1, 50, 0.5);
        assert_eq!(fee, 100); // Floor at min_fee
    }

    #[test]
    fn test_calculate_fee_multi_operation() {
        let engine = create_test_engine();
        let fee = engine.calculate_fee_with_multiplier(5, 100, 1.0);
        assert_eq!(fee, 500);
    }
}
