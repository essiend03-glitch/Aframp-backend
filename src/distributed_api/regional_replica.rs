use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Geographic region identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Region {
    UsEast,
    UsWest,
    EuWest,
    EuCentral,
    ApSoutheast,
    ApNortheast,
    AfricaSouth,
    SouthAmerica,
}

impl Region {
    pub fn as_str(&self) -> &'static str {
        match self {
            Region::UsEast => "us-east-1",
            Region::UsWest => "us-west-2",
            Region::EuWest => "eu-west-1",
            Region::EuCentral => "eu-central-1",
            Region::ApSoutheast => "ap-southeast-1",
            Region::ApNortheast => "ap-northeast-1",
            Region::AfricaSouth => "af-south-1",
            Region::SouthAmerica => "sa-east-1",
        }
    }

    pub fn latency_ms(&self) -> u32 {
        // Typical inter-region latencies
        match self {
            Region::UsEast => 0,
            Region::UsWest => 50,
            Region::EuWest => 80,
            Region::EuCentral => 85,
            Region::ApSoutheast => 150,
            Region::ApNortheast => 160,
            Region::AfricaSouth => 120,
            Region::SouthAmerica => 140,
        }
    }
}

/// Configuration for a regional replica
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaConfig {
    pub region: Region,
    pub db_read_replica_url: String,
    pub max_replication_lag_ms: u32,
    pub health_check_interval: Duration,
    pub request_timeout: Duration,
}

/// Health status of a regional replica
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicaHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Metrics for a regional replica
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaMetrics {
    pub region: Region,
    pub health: ReplicaHealth,
    pub replication_lag_ms: u32,
    pub error_rate: f64,
    pub p99_latency_ms: u32,
    pub last_health_check: SystemTime,
    pub requests_served: u64,
}

/// Regional API replica instance
pub struct RegionalReplica {
    config: ReplicaConfig,
    metrics: ReplicaMetrics,
}

impl RegionalReplica {
    pub fn new(config: ReplicaConfig) -> Self {
        Self {
            config: config.clone(),
            metrics: ReplicaMetrics {
                region: config.region,
                health: ReplicaHealth::Healthy,
                replication_lag_ms: 0,
                error_rate: 0.0,
                p99_latency_ms: 0,
                last_health_check: SystemTime::now(),
                requests_served: 0,
            },
        }
    }

    pub fn region(&self) -> Region {
        self.config.region
    }

    pub fn is_healthy(&self) -> bool {
        self.metrics.health == ReplicaHealth::Healthy
    }

    pub fn replication_lag_ms(&self) -> u32 {
        self.metrics.replication_lag_ms
    }

    pub fn update_metrics(&mut self, lag_ms: u32, error_rate: f64, p99_latency_ms: u32) {
        self.metrics.replication_lag_ms = lag_ms;
        self.metrics.error_rate = error_rate;
        self.metrics.p99_latency_ms = p99_latency_ms;
        self.metrics.last_health_check = SystemTime::now();

        // Determine health status
        self.metrics.health = if lag_ms > self.config.max_replication_lag_ms {
            ReplicaHealth::Unhealthy
        } else if error_rate > 0.05 || p99_latency_ms > 1000 {
            ReplicaHealth::Degraded
        } else {
            ReplicaHealth::Healthy
        };
    }

    pub fn record_request(&mut self) {
        self.metrics.requests_served += 1;
    }

    pub fn metrics(&self) -> &ReplicaMetrics {
        &self.metrics
    }

    pub fn config(&self) -> &ReplicaConfig {
        &self.config
    }
}

/// Registry of all regional replicas
pub struct ReplicaRegistry {
    replicas: Vec<RegionalReplica>,
}

impl ReplicaRegistry {
    pub fn new() -> Self {
        Self {
            replicas: Vec::new(),
        }
    }

    pub fn register(&mut self, replica: RegionalReplica) {
        self.replicas.push(replica);
    }

    pub fn get_healthy_replicas(&self) -> Vec<&RegionalReplica> {
        self.replicas.iter()
            .filter(|r| r.is_healthy())
            .collect()
    }

    pub fn get_replica_by_region(&self, region: Region) -> Option<&RegionalReplica> {
        self.replicas.iter().find(|r| r.region() == region)
    }

    pub fn get_all_replicas(&self) -> &[RegionalReplica] {
        &self.replicas
    }

    pub fn get_all_replicas_mut(&mut self) -> &mut [RegionalReplica] {
        &mut self.replicas
    }
}

impl Default for ReplicaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
