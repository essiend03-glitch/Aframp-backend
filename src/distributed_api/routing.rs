use crate::distributed_api::regional_replica::{Region, ReplicaRegistry};
use crate::distributed_api::consistency::ConsistencyLevel;
use serde::{Deserialize, Serialize};

/// IP geolocation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    pub country: String,
    pub region: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// Routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub target_region: Region,
    pub use_cache: bool,
    pub consistency_level: ConsistencyLevel,
    pub reason: String,
}

/// Geographic router for request routing
pub struct GeoRouter {
    replica_registry: ReplicaRegistry,
}

impl GeoRouter {
    pub fn new(replica_registry: ReplicaRegistry) -> Self {
        Self { replica_registry }
    }

    /// Route request based on client IP and consistency requirements
    pub fn route_request(
        &self,
        client_ip: &str,
        consistency: ConsistencyLevel,
    ) -> RoutingDecision {
        // Determine target region based on consistency level
        match consistency {
            ConsistencyLevel::Strong => {
                // Always route to primary (US-East)
                RoutingDecision {
                    target_region: Region::UsEast,
                    use_cache: false,
                    consistency_level: ConsistencyLevel::Strong,
                    reason: "Strong consistency requires primary region".to_string(),
                }
            }
            ConsistencyLevel::Eventual => {
                // Route to closest healthy replica
                let target = self.find_closest_healthy_replica(client_ip);
                RoutingDecision {
                    target_region: target,
                    use_cache: true,
                    consistency_level: ConsistencyLevel::Eventual,
                    reason: format!("Routed to closest replica: {}", target.as_str()),
                }
            }
            ConsistencyLevel::ReadAfterWrite => {
                // Route to closest replica, fallback to primary if needed
                let target = self.find_closest_healthy_replica(client_ip);
                RoutingDecision {
                    target_region: target,
                    use_cache: false,
                    consistency_level: ConsistencyLevel::ReadAfterWrite,
                    reason: format!("Read-after-write routed to: {}", target.as_str()),
                }
            }
        }
    }

    /// Find closest healthy replica to client
    fn find_closest_healthy_replica(&self, _client_ip: &str) -> Region {
        let healthy = self.replica_registry.get_healthy_replicas();
        
        if healthy.is_empty() {
            // Fallback to primary if no healthy replicas
            return Region::UsEast;
        }

        // In production: use MaxMind GeoIP2 to determine client location
        // For now, return first healthy replica
        healthy[0].region()
    }

    /// Get estimated latency to region
    pub fn estimate_latency(&self, region: Region) -> u32 {
        region.latency_ms()
    }

    /// Get all available regions
    pub fn available_regions(&self) -> Vec<Region> {
        self.replica_registry.get_all_replicas()
            .iter()
            .map(|r| r.region())
            .collect()
    }

    /// Get healthy regions
    pub fn healthy_regions(&self) -> Vec<Region> {
        self.replica_registry.get_healthy_replicas()
            .iter()
            .map(|r| r.region())
            .collect()
    }
}

/// Latency-based routing strategy
pub struct LatencyBasedRouter {
    router: GeoRouter,
}

impl LatencyBasedRouter {
    pub fn new(router: GeoRouter) -> Self {
        Self { router }
    }

    /// Route to region with lowest latency
    pub fn route_to_lowest_latency(&self) -> Region {
        let regions = self.router.available_regions();
        
        regions.into_iter()
            .min_by_key(|r| self.router.estimate_latency(*r))
            .unwrap_or(Region::UsEast)
    }
}

/// Anycast routing strategy
pub struct AnycastRouter {
    router: GeoRouter,
}

impl AnycastRouter {
    pub fn new(router: GeoRouter) -> Self {
        Self { router }
    }

    /// Route using anycast - client connects to nearest healthy instance
    pub fn route_anycast(&self) -> Region {
        let healthy = self.router.healthy_regions();
        
        if healthy.is_empty() {
            Region::UsEast
        } else {
            healthy[0]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_decision() {
        let registry = ReplicaRegistry::new();
        let router = GeoRouter::new(registry);

        let decision = router.route_request("192.0.2.1", ConsistencyLevel::Strong);
        assert_eq!(decision.target_region, Region::UsEast);
        assert!(!decision.use_cache);

        let decision = router.route_request("192.0.2.1", ConsistencyLevel::Eventual);
        assert!(decision.use_cache);
    }

    #[test]
    fn test_latency_estimation() {
        let registry = ReplicaRegistry::new();
        let router = GeoRouter::new(registry);

        let latency = router.estimate_latency(Region::UsEast);
        assert_eq!(latency, 0);

        let latency = router.estimate_latency(Region::EuWest);
        assert_eq!(latency, 80);
    }
}
