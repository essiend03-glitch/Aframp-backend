//! Multi-Region Deployment & Geographic Distribution
//!
//! Implements comprehensive multi-region architecture with:
//! - Active-Active/Active-Passive orchestration
//! - Global load balancing and DNS routing
//! - Cross-region database replication
//! - State synchronization and data sovereignty
//! - Infrastructure as Code portability
//! - Global observability and monitoring

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use anyhow::Result;
use tracing::{info, warn, error};

pub mod load_balancer;
pub mod database_replication;
pub mod state_sync;
pub mod infrastructure;
pub mod monitoring;
pub mod failover;

pub use load_balancer::*;
pub use database_replication::*;
pub use state_sync::*;
pub use infrastructure::*;
pub use monitoring::*;
pub use failover::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRegionConfig {
    pub primary_region: String,
    pub regions: Vec<RegionConfig>,
    pub database: DatabaseConfig,
    pub load_balancing: LoadBalancingConfig,
    pub failover: FailoverConfig,
    pub monitoring: GlobalMonitoringConfig,
    pub infrastructure: InfrastructureConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionConfig {
    pub name: String,
    pub region_code: String,
    pub endpoint: String,
    pub database_url: String,
    pub redis_url: String,
    pub priority: u8, // 1 = highest priority
    pub capacity: RegionCapacity,
    pub location: GeographicLocation,
    pub compliance: ComplianceRequirements,
    pub status: RegionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionCapacity {
    pub max_connections: u32,
    pub max_requests_per_second: u32,
    pub cpu_cores: u32,
    pub memory_gb: u32,
    pub storage_gb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicLocation {
    pub country: String,
    pub continent: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRequirements {
    pub data_residency_required: bool,
    pub gdpr_applicable: bool,
    pub local_storage_required: bool,
    pub audit_retention_days: u32,
    pub encryption_at_rest_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegionStatus {
    Active,
    Draining, // Traffic being redirected away
    Maintenance,
    Failed,
    Standby,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub replication_mode: ReplicationMode,
    pub primary_region: String,
    pub replica_regions: Vec<String>,
    pub replication_lag_target_ms: u64,
    pub failover_mode: DatabaseFailoverMode,
    pub consistency_level: ConsistencyLevel,
    pub backup_strategy: BackupStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplicationMode {
    Asynchronous,
    Synchronous,
    SemiSynchronous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseFailoverMode {
    Automatic,
    Manual,
    Scheduled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsistencyLevel {
    Strong,
    Eventual,
    BoundedStaleness(Duration),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupStrategy {
    pub continuous_backup: bool,
    pub point_in_time_recovery: bool,
    pub cross_region_backup: bool,
    pub backup_retention_days: u32,
    pub backup_frequency_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    pub strategy: LoadBalancingStrategy,
    pub health_check_interval: Duration,
    pub health_check_timeout: Duration,
    pub unhealthy_threshold: u32,
    pub healthy_threshold: u32,
    pub dns_ttl_seconds: u32,
    pub geo_routing: GeoRoutingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    ResponseTime,
    Geographic,
    HealthBased,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoRoutingConfig {
    pub enabled: bool,
    pub country_routing: HashMap<String, String>, // country -> region
    pub latency_routing: bool,
    pub failover_routing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    pub automatic_failover: bool,
    pub failover_timeout: Duration,
    pub health_check_interval: Duration,
    pub max_failures_before_failover: u32,
    pub recovery_strategy: RecoveryStrategy,
    pub notification_channels: Vec<NotificationChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    Automatic,
    ManualApproval,
    Gradual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    Email(String),
    Slack(String),
    PagerDuty(String),
    Webhook(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMonitoringConfig {
    pub centralized_logging: bool,
    pub metrics_aggregation: bool,
    pub alert_routing: AlertRoutingConfig,
    pub dashboard_regions: Vec<String>,
    pub retention_period_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRoutingConfig {
    pub regional_alerts: bool,
    pub global_alerts: bool,
    pub severity_routing: HashMap<String, Vec<String>>, // severity -> regions
    pub escalation_rules: Vec<EscalationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRule {
    pub alert_type: String,
    pub threshold_duration: Duration,
    pub escalation_level: u8,
    pub notification_channels: Vec<NotificationChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureConfig {
    pub as_code: bool,
    pub terraform_enabled: bool,
    pub pulumi_enabled: bool,
    pub deployment_strategy: DeploymentStrategy,
    pub environment_isolation: bool,
    pub secret_management: SecretManagementConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStrategy {
    BlueGreen,
    Rolling,
    Canary,
    Recreate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretManagementConfig {
    pub provider: SecretProvider,
    pub cross_region_sync: bool,
    pub rotation_enabled: bool,
    pub rotation_period_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretProvider {
    AwsSecretsManager,
    AzureKeyVault,
    HashiCorpVault,
    GoogleSecretManager,
}

impl Default for MultiRegionConfig {
    fn default() -> Self {
        Self {
            primary_region: "us-east-1".to_string(),
            regions: vec![
                RegionConfig {
                    name: "US East".to_string(),
                    region_code: "us-east-1".to_string(),
                    endpoint: "https://api.aframp.com".to_string(),
                    database_url: "postgresql://...".to_string(),
                    redis_url: "redis://...".to_string(),
                    priority: 1,
                    capacity: RegionCapacity {
                        max_connections: 1000,
                        max_requests_per_second: 5000,
                        cpu_cores: 16,
                        memory_gb: 64,
                        storage_gb: 1000,
                    },
                    location: GeographicLocation {
                        country: "US".to_string(),
                        continent: "North America".to_string(),
                        latitude: 37.7749,
                        longitude: -122.4194,
                        timezone: "America/New_York".to_string(),
                    },
                    compliance: ComplianceRequirements {
                        data_residency_required: false,
                        gdpr_applicable: false,
                        local_storage_required: false,
                        audit_retention_days: 2555, // 7 years
                        encryption_at_rest_required: true,
                    },
                    status: RegionStatus::Active,
                },
                RegionConfig {
                    name: "EU West".to_string(),
                    region_code: "eu-west-1".to_string(),
                    endpoint: "https://api.eu.aframp.com".to_string(),
                    database_url: "postgresql://...".to_string(),
                    redis_url: "redis://...".to_string(),
                    priority: 2,
                    capacity: RegionCapacity {
                        max_connections: 800,
                        max_requests_per_second: 4000,
                        cpu_cores: 12,
                        memory_gb: 48,
                        storage_gb: 800,
                    },
                    location: GeographicLocation {
                        country: "IE".to_string(),
                        continent: "Europe".to_string(),
                        latitude: 53.3498,
                        longitude: -6.2603,
                        timezone: "Europe/Dublin".to_string(),
                    },
                    compliance: ComplianceRequirements {
                        data_residency_required: true,
                        gdpr_applicable: true,
                        local_storage_required: true,
                        audit_retention_days: 2555,
                        encryption_at_rest_required: true,
                    },
                    status: RegionStatus::Active,
                },
                RegionConfig {
                    name: "Africa South".to_string(),
                    region_code: "af-south-1".to_string(),
                    endpoint: "https://api.af.aframp.com".to_string(),
                    database_url: "postgresql://...".to_string(),
                    redis_url: "redis://...".to_string(),
                    priority: 3,
                    capacity: RegionCapacity {
                        max_connections: 600,
                        max_requests_per_second: 3000,
                        cpu_cores: 8,
                        memory_gb: 32,
                        storage_gb: 500,
                    },
                    location: GeographicLocation {
                        country: "ZA".to_string(),
                        continent: "Africa".to_string(),
                        latitude: -26.2041,
                        longitude: 28.0473,
                        timezone: "Africa/Johannesburg".to_string(),
                    },
                    compliance: ComplianceRequirements {
                        data_residency_required: true,
                        gdpr_applicable: false,
                        local_storage_required: true,
                        audit_retention_days: 2555,
                        encryption_at_rest_required: true,
                    },
                    status: RegionStatus::Active,
                },
            ],
            database: DatabaseConfig {
                replication_mode: ReplicationMode::Asynchronous,
                primary_region: "us-east-1".to_string(),
                replica_regions: vec!["eu-west-1".to_string(), "af-south-1".to_string()],
                replication_lag_target_ms: 500,
                failover_mode: DatabaseFailoverMode::Automatic,
                consistency_level: ConsistencyLevel::BoundedStaleness(Duration::from_secs(1)),
                backup_strategy: BackupStrategy {
                    continuous_backup: true,
                    point_in_time_recovery: true,
                    cross_region_backup: true,
                    backup_retention_days: 30,
                    backup_frequency_hours: 6,
                },
            },
            load_balancing: LoadBalancingConfig {
                strategy: LoadBalancingStrategy::Geographic,
                health_check_interval: Duration::from_secs(30),
                health_check_timeout: Duration::from_secs(5),
                unhealthy_threshold: 3,
                healthy_threshold: 2,
                dns_ttl_seconds: 60,
                geo_routing: GeoRoutingConfig {
                    enabled: true,
                    country_routing: {
                        "US".to_string() => "us-east-1".to_string(),
                        "CA".to_string() => "us-east-1".to_string(),
                        "GB".to_string() => "eu-west-1".to_string(),
                        "DE".to_string() => "eu-west-1".to_string(),
                        "FR".to_string() => "eu-west-1".to_string(),
                        "NG".to_string() => "af-south-1".to_string(),
                        "ZA".to_string() => "af-south-1".to_string(),
                        "KE".to_string() => "af-south-1".to_string(),
                    }.into_iter().collect(),
                    latency_routing: true,
                    failover_routing: true,
                },
            },
            failover: FailoverConfig {
                automatic_failover: true,
                failover_timeout: Duration::from_secs(60),
                health_check_interval: Duration::from_secs(10),
                max_failures_before_failover: 3,
                recovery_strategy: RecoveryStrategy::Automatic,
                notification_channels: vec![
                    NotificationChannel::Email("alerts@aframp.com".to_string()),
                    NotificationChannel::Slack("#devops-alerts".to_string()),
                    NotificationChannel::PagerDuty("aframp-pagerduty".to_string()),
                ],
            },
            monitoring: GlobalMonitoringConfig {
                centralized_logging: true,
                metrics_aggregation: true,
                alert_routing: AlertRoutingConfig {
                    regional_alerts: true,
                    global_alerts: true,
                    severity_routing: {
                        "critical".to_string() => vec!["us-east-1".to_string(), "eu-west-1".to_string()],
                        "warning".to_string() => vec!["us-east-1".to_string()],
                        "info".to_string() => vec![],
                    }.into_iter().collect(),
                    escalation_rules: vec![
                        EscalationRule {
                            alert_type: "region_down".to_string(),
                            threshold_duration: Duration::from_secs(300), // 5 minutes
                            escalation_level: 1,
                            notification_channels: vec![
                                NotificationChannel::Slack("#devops-alerts".to_string()),
                            ],
                        },
                        EscalationRule {
                            alert_type: "database_replication_lag".to_string(),
                            threshold_duration: Duration::from_secs(600), // 10 minutes
                            escalation_level: 2,
                            notification_channels: vec![
                                NotificationChannel::Email("dba@aframp.com".to_string()),
                                NotificationChannel::PagerDuty("aframp-pagerduty".to_string()),
                            ],
                        },
                    ],
                },
                dashboard_regions: vec!["us-east-1".to_string()],
                retention_period_days: 30,
            },
            infrastructure: InfrastructureConfig {
                as_code: true,
                terraform_enabled: true,
                pulumi_enabled: false,
                deployment_strategy: DeploymentStrategy::BlueGreen,
                environment_isolation: true,
                secret_management: SecretManagementConfig {
                    provider: SecretProvider::AwsSecretsManager,
                    cross_region_sync: true,
                    rotation_enabled: true,
                    rotation_period_days: 90,
                },
            },
        }
    }
}

impl MultiRegionConfig {
    pub fn from_env() -> Result<Self> {
        // Load configuration from environment variables
        let primary_region = std::env::var("PRIMARY_REGION")
            .unwrap_or_else(|_| "us-east-1".to_string());

        let mut config = Self::default();
        config.primary_region = primary_region;

        // Override with environment variables if present
        if let Ok(val) = std::env::var("DATABASE_REPLICATION_LAG_MS") {
            config.database.replication_lag_target_ms = val.parse()?;
        }

        if let Ok(val) = std::env::var("FAILOVER_TIMEOUT_SECONDS") {
            config.failover.failover_timeout = Duration::from_secs(val.parse()?);
        }

        if let Ok(val) = std::env::var("AUTO_FAILOVER_ENABLED") {
            config.failover.automatic_failover = val.parse()?;
        }

        Ok(config)
    }

    pub fn get_region_by_code(&self, region_code: &str) -> Option<&RegionConfig> {
        self.regions.iter().find(|r| r.region_code == region_code)
    }

    pub fn get_active_regions(&self) -> Vec<&RegionConfig> {
        self.regions.iter()
            .filter(|r| matches!(r.status, RegionStatus::Active))
            .collect()
    }

    pub fn get_primary_region_config(&self) -> Option<&RegionConfig> {
        self.get_region_by_code(&self.primary_region)
    }

    pub fn calculate_total_capacity(&self) -> RegionCapacity {
        let mut total = RegionCapacity {
            max_connections: 0,
            max_requests_per_second: 0,
            cpu_cores: 0,
            memory_gb: 0,
            storage_gb: 0,
        };

        for region in &self.regions {
            total.max_connections += region.capacity.max_connections;
            total.max_requests_per_second += region.capacity.max_requests_per_second;
            total.cpu_cores += region.capacity.cpu_cores;
            total.memory_gb += region.capacity.memory_gb;
            total.storage_gb += region.capacity.storage_gb;
        }

        total
    }

    pub fn validate(&self) -> Result<()> {
        // Validate primary region exists
        if self.get_primary_region_config().is_none() {
            return Err(anyhow::anyhow!("Primary region {} not found in configuration", self.primary_region));
        }

        // Validate at least one active region
        if self.get_active_regions().is_empty() {
            return Err(anyhow::anyhow!("No active regions configured"));
        }

        // Validate database replication configuration
        if !self.database.replica_regions.contains(&self.database.primary_region) {
            if self.database.replica_regions.is_empty() {
                warn!("No database replica regions configured - single point of failure risk");
            }
        }

        // Validate load balancing configuration
        if self.load_balancing.geo_routing.enabled && self.load_balancing.geo_routing.country_routing.is_empty() {
            return Err(anyhow::anyhow!("Geographic routing enabled but no country routing configured"));
        }

        // Validate failover configuration
        if self.failover.automatic_failover && self.failover.max_failures_before_failover == 0 {
            return Err(anyhow::anyhow!("Automatic failover enabled but failure threshold is 0"));
        }

        info!("Multi-region configuration validation passed");
        Ok(())
    }

    pub fn get_region_for_country(&self, country: &str) -> Option<&RegionConfig> {
        if let Some(region_code) = self.load_balancing.geo_routing.country_routing.get(country) {
            self.get_region_by_code(region_code)
        } else {
            // Fallback to primary region
            self.get_primary_region_config()
        }
    }

    pub fn estimate_failover_time(&self) -> Duration {
        // Estimate failover time based on configuration
        let base_time = self.failover.failover_timeout;
        let health_check_time = self.failover.health_check_interval;
        let dns_propagation_time = Duration::from_secs(self.load_balancing.dns_ttl_seconds);

        base_time + health_check_time + dns_propagation_time
    }

    pub fn get_compliance_summary(&self) -> HashMap<String, Vec<String>> {
        let mut summary = HashMap::new();

        for region in &self.regions {
            let mut requirements = Vec::new();

            if region.compliance.data_residency_required {
                requirements.push("Data Residency".to_string());
            }
            if region.compliance.gdpr_applicable {
                requirements.push("GDPR".to_string());
            }
            if region.compliance.local_storage_required {
                requirements.push("Local Storage".to_string());
            }
            if region.compliance.encryption_at_rest_required {
                requirements.push("Encryption at Rest".to_string());
            }

            summary.insert(region.region_code.clone(), requirements);
        }

        summary
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiRegionStatus {
    pub primary_region: String,
    pub active_regions: Vec<String>,
    pub failed_regions: Vec<String>,
    pub maintenance_regions: Vec<String>,
    pub database_replication_status: DatabaseReplicationStatus,
    pub load_balancer_status: LoadBalancerStatus,
    pub overall_health: HealthStatus,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseReplicationStatus {
    pub primary_region: String,
    pub replica_status: HashMap<String, ReplicaStatus>,
    pub replication_lag_ms: HashMap<String, u64>,
    pub last_failover: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaStatus {
    pub region: String,
    pub status: ReplicaHealth,
    pub lag_ms: u64,
    pub last_sync: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplicaHealth {
    Healthy,
    Lagging,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerStatus {
    pub strategy: LoadBalancingStrategy,
    pub healthy_endpoints: Vec<String>,
    pub unhealthy_endpoints: Vec<String>,
    pub total_requests_per_second: u32,
    pub average_response_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_region_config_default() {
        let config = MultiRegionConfig::default();
        assert_eq!(config.primary_region, "us-east-1");
        assert_eq!(config.regions.len(), 3);
        assert!(config.get_primary_region_config().is_some());
    }

    #[test]
    fn test_region_lookup() {
        let config = MultiRegionConfig::default();
        
        assert!(config.get_region_by_code("us-east-1").is_some());
        assert!(config.get_region_by_code("invalid-region").is_none());
        
        assert!(config.get_region_for_country("US").is_some());
        assert_eq!(config.get_region_for_country("US").unwrap().region_code, "us-east-1");
    }

    #[test]
    fn test_config_validation() {
        let config = MultiRegionConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_capacity_calculation() {
        let config = MultiRegionConfig::default();
        let total = config.calculate_total_capacity();
        
        assert!(total.max_connections > 0);
        assert!(total.cpu_cores > 0);
        assert!(total.memory_gb > 0);
    }

    #[test]
    fn test_compliance_summary() {
        let config = MultiRegionConfig::default();
        let summary = config.get_compliance_summary();
        
        assert!(summary.contains_key("us-east-1"));
        assert!(summary.contains_key("eu-west-1"));
        assert!(summary.contains_key("af-south-1"));
        
        // EU region should have GDPR
        let eu_requirements = summary.get("eu-west-1").unwrap();
        assert!(eu_requirements.contains(&"GDPR".to_string()));
    }
}
