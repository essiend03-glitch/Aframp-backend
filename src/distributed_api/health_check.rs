use crate::distributed_api::regional_replica::{Region, ReplicaHealth};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub interval: Duration,
    pub timeout: Duration,
    pub unhealthy_threshold: u32,
    pub healthy_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            unhealthy_threshold: 3,
            healthy_threshold: 2,
        }
    }
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub region: Region,
    pub healthy: bool,
    pub replication_lag_ms: u32,
    pub error_rate: f64,
    pub p99_latency_ms: u32,
    pub timestamp: SystemTime,
    pub details: String,
}

/// Global health checker
pub struct HealthChecker {
    config: HealthCheckConfig,
    last_checks: std::collections::HashMap<Region, Vec<bool>>,
}

impl HealthChecker {
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            last_checks: std::collections::HashMap::new(),
        }
    }

    /// Perform health check for a region
    pub async fn check_region(&mut self, region: Region) -> HealthCheckResult {
        // Simulate health check - in production, this would:
        // 1. Query the regional replica's health endpoint
        // 2. Check replication lag from primary
        // 3. Measure response latency
        // 4. Calculate error rate from recent requests

        let replication_lag_ms = self.simulate_replication_lag(region);
        let error_rate = self.simulate_error_rate(region);
        let p99_latency_ms = self.simulate_latency(region);

        let healthy = replication_lag_ms < 5000 && error_rate < 0.05;

        // Track health history
        let checks = self.last_checks.entry(region).or_insert_with(Vec::new);
        checks.push(healthy);
        if checks.len() > self.config.unhealthy_threshold as usize {
            checks.remove(0);
        }

        HealthCheckResult {
            region,
            healthy,
            replication_lag_ms,
            error_rate,
            p99_latency_ms,
            timestamp: SystemTime::now(),
            details: format!(
                "Lag: {}ms, Error: {:.2}%, P99: {}ms",
                replication_lag_ms, error_rate * 100.0, p99_latency_ms
            ),
        }
    }

    /// Determine if region should be considered healthy
    pub fn is_region_healthy(&self, region: Region) -> bool {
        if let Some(checks) = self.last_checks.get(&region) {
            let recent_healthy = checks.iter().rev()
                .take(self.config.healthy_threshold as usize)
                .filter(|&&h| h)
                .count();
            
            recent_healthy >= self.config.healthy_threshold as usize
        } else {
            false
        }
    }

    /// Get health status for all regions
    pub fn get_all_health_status(&self) -> Vec<(Region, bool)> {
        vec![
            (Region::UsEast, self.is_region_healthy(Region::UsEast)),
            (Region::UsWest, self.is_region_healthy(Region::UsWest)),
            (Region::EuWest, self.is_region_healthy(Region::EuWest)),
            (Region::EuCentral, self.is_region_healthy(Region::EuCentral)),
            (Region::ApSoutheast, self.is_region_healthy(Region::ApSoutheast)),
            (Region::ApNortheast, self.is_region_healthy(Region::ApNortheast)),
            (Region::AfricaSouth, self.is_region_healthy(Region::AfricaSouth)),
            (Region::SouthAmerica, self.is_region_healthy(Region::SouthAmerica)),
        ]
    }

    fn simulate_replication_lag(&self, _region: Region) -> u32 {
        // In production: query actual replication lag from database
        rand::random::<u32>() % 1000
    }

    fn simulate_error_rate(&self, _region: Region) -> f64 {
        // In production: calculate from request metrics
        (rand::random::<u32>() % 100) as f64 / 10000.0
    }

    fn simulate_latency(&self, region: Region) -> u32 {
        // In production: measure actual p99 latency
        region.latency_ms() + (rand::random::<u32>() % 50)
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new(HealthCheckConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let mut checker = HealthChecker::new(HealthCheckConfig::default());
        let result = checker.check_region(Region::UsEast).await;
        
        assert_eq!(result.region, Region::UsEast);
        assert!(result.timestamp <= SystemTime::now());
    }
}
