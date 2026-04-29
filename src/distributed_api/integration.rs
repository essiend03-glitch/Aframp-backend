/// Integration module for distributed API components
/// 
/// This module ties together edge caching, regional replicas, consistency management,
/// health checking, and routing into a cohesive distributed system.

use crate::distributed_api::{
    EdgeCacheLayer, RegionalReplica, ConsistencyManager, HealthChecker, GeoRouter,
};
use crate::distributed_api::regional_replica::{Region, ReplicaRegistry, ReplicaConfig};
use crate::distributed_api::edge_cache::EdgeCacheConfig;
use crate::distributed_api::health_check::HealthCheckConfig;
use crate::distributed_api::consistency::ConsistencyLevel;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Global distributed API system
pub struct DistributedApiSystem {
    edge_cache: EdgeCacheLayer,
    replica_registry: ReplicaRegistry,
    consistency_manager: ConsistencyManager,
    health_checker: HealthChecker,
    geo_router: GeoRouter,
}

impl DistributedApiSystem {
    pub fn new(
        edge_cache_config: EdgeCacheConfig,
        health_check_config: HealthCheckConfig,
    ) -> Self {
        let replica_registry = ReplicaRegistry::new();
        let consistency_manager = ConsistencyManager::new();
        let health_checker = HealthChecker::new(health_check_config);
        let geo_router = GeoRouter::new(replica_registry.clone());

        Self {
            edge_cache: EdgeCacheLayer::new(edge_cache_config),
            replica_registry,
            consistency_manager,
            health_checker,
            geo_router,
        }
    }

    /// Initialize with default configuration
    pub fn with_defaults() -> Self {
        Self::new(
            EdgeCacheConfig::default(),
            HealthCheckConfig::default(),
        )
    }

    /// Register a regional replica
    pub fn register_replica(&mut self, config: ReplicaConfig) {
        let replica = RegionalReplica::new(config);
        self.replica_registry.register(replica);
    }

    /// Get routing decision for a request
    pub fn get_routing_decision(&self, client_ip: &str, path: &str) -> RoutingDecision {
        let policy = self.consistency_manager.get_policy(path);
        let consistency = policy
            .map(|p| p.consistency)
            .unwrap_or(ConsistencyLevel::Eventual);

        let routing = self.geo_router.route_request(client_ip, consistency);

        RoutingDecision {
            target_region: routing.target_region,
            use_cache: routing.use_cache,
            consistency_level: consistency,
            cache_ttl: policy.and_then(|p| p.cache_ttl_seconds),
            reason: routing.reason,
        }
    }

    /// Cache a response
    pub fn cache_response(&mut self, key: String, data: String, ttl: Option<Duration>) {
        self.edge_cache.cache_response(key, data, ttl);
    }

    /// Get cached response
    pub fn get_cached_response(&self, key: &str) -> Option<(String, bool)> {
        self.edge_cache.get_response(key)
            .map(|(resp, is_stale)| (resp.data, is_stale))
    }

    /// Invalidate cache entry
    pub fn invalidate_cache(&mut self, key: &str) {
        self.edge_cache.invalidate(key);
    }

    /// Invalidate cache by pattern
    pub fn invalidate_cache_pattern(&mut self, pattern: &str) {
        self.edge_cache.invalidate_pattern(pattern);
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let stats = self.edge_cache.stats();
        CacheStats {
            entries: stats.entries,
            size_bytes: stats.size_bytes,
            max_size_bytes: stats.max_size_bytes,
        }
    }

    /// Get health status of all regions
    pub fn get_health_status(&self) -> Vec<(Region, bool)> {
        self.health_checker.get_all_health_status()
    }

    /// Get healthy regions
    pub fn get_healthy_regions(&self) -> Vec<Region> {
        self.geo_router.healthy_regions()
    }

    /// Get all available regions
    pub fn get_all_regions(&self) -> Vec<Region> {
        self.geo_router.available_regions()
    }

    /// Get consistency policies
    pub fn get_consistency_policies(&self) -> Vec<ConsistencyPolicyInfo> {
        self.consistency_manager.list_policies()
            .iter()
            .map(|p| ConsistencyPolicyInfo {
                path: p.path.clone(),
                consistency: p.consistency,
                cache_ttl_seconds: p.cache_ttl_seconds,
                description: p.description.clone(),
            })
            .collect()
    }
}

/// Routing decision information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub target_region: Region,
    pub use_cache: bool,
    pub consistency_level: ConsistencyLevel,
    pub cache_ttl: Option<u64>,
    pub reason: String,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub entries: usize,
    pub size_bytes: usize,
    pub max_size_bytes: usize,
}

/// Consistency policy information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyPolicyInfo {
    pub path: String,
    pub consistency: ConsistencyLevel,
    pub cache_ttl_seconds: Option<u64>,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distributed_api_system_creation() {
        let system = DistributedApiSystem::with_defaults();
        assert_eq!(system.get_all_regions().len(), 0); // No replicas registered yet
    }

    #[test]
    fn test_cache_operations() {
        let mut system = DistributedApiSystem::with_defaults();
        
        system.cache_response(
            "test_key".to_string(),
            "test_data".to_string(),
            Some(Duration::from_secs(300)),
        );

        let cached = system.get_cached_response("test_key");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().0, "test_data");

        system.invalidate_cache("test_key");
        assert!(system.get_cached_response("test_key").is_none());
    }

    #[test]
    fn test_routing_decision() {
        let system = DistributedApiSystem::with_defaults();
        
        let decision = system.get_routing_decision("192.0.2.1", "/api/v1/ledger/snapshots");
        assert!(decision.use_cache);
        assert_eq!(decision.consistency_level, ConsistencyLevel::Eventual);

        let decision = system.get_routing_decision("192.0.2.1", "/api/v1/transactions/sign");
        assert!(!decision.use_cache);
        assert_eq!(decision.consistency_level, ConsistencyLevel::Strong);
    }
}
