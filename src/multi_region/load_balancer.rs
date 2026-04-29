//! Global Load Balancing for Multi-Region Deployment
//!
//! Implements intelligent traffic distribution across regions:
//! - DNS-based global routing
//! - Geographic load balancing
//! - Health-based routing
//! - Latency optimization
//! - Automatic failover and recovery

use super::{MultiRegionConfig, RegionConfig, RegionStatus};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct GlobalLoadBalancer {
    config: MultiRegionConfig,
    region_health: HashMap<String, RegionHealth>,
    routing_table: HashMap<String, VecDeque<String>>, // country -> region queue
    health_checker: HealthChecker,
    dns_manager: DNSManager,
}

#[derive(Debug, Clone)]
pub struct RegionHealth {
    pub region_code: String,
    pub status: HealthStatus,
    pub response_time_ms: u64,
    pub success_rate: f64,
    pub requests_per_second: u32,
    pub error_rate: f64,
    pub last_health_check: Instant,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct HealthChecker {
    config: HealthCheckConfig,
    active_regions: HashMap<String, RegionHealth>,
}

#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub interval: Duration,
    pub timeout: Duration,
    pub unhealthy_threshold: u32,
    pub healthy_threshold: u32,
    pub check_endpoints: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DNSManager {
    config: DNSConfig,
    current_records: HashMap<String, DNSRecord>,
}

#[derive(Debug, Clone)]
pub struct DNSConfig {
    pub provider: DNSProvider,
    pub ttl_seconds: u32,
    pub failover_enabled: bool,
    pub geo_routing_enabled: bool,
}

#[derive(Debug, Clone)]
pub enum DNSProvider {
    Route53,
    Cloudflare,
    AzureDNS,
    GoogleDNS,
}

#[derive(Debug, Clone)]
pub struct DNSRecord {
    pub name: String,
    pub values: Vec<String>,
    pub ttl: u32,
    pub geo_routing: HashMap<String, String>, // country -> value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub selected_region: String,
    pub endpoint: String,
    pub routing_reason: RoutingReason,
    pub estimated_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingReason {
    Geographic,
    Latency,
    LoadBalancing,
    HealthBased,
    Failover,
    Default,
}

impl GlobalLoadBalancer {
    pub fn new(config: MultiRegionConfig) -> Self {
        let mut region_health = HashMap::new();
        let mut routing_table = HashMap::new();

        // Initialize region health tracking
        for region in &config.regions {
            region_health.insert(
                region.region_code.clone(),
                RegionHealth {
                    region_code: region.region_code.clone(),
                    status: HealthStatus::Unknown,
                    response_time_ms: 0,
                    success_rate: 1.0,
                    requests_per_second: 0,
                    error_rate: 0.0,
                    last_health_check: Instant::now(),
                    consecutive_failures: 0,
                    consecutive_successes: 0,
                },
            );
        }

        // Initialize geographic routing table
        if config.load_balancing.geo_routing.enabled {
            for (country, region_code) in &config.load_balancing.geo_routing.country_routing {
                let queue = routing_table.entry(country.clone()).or_insert_with(VecDeque::new);
                queue.push_back(region_code.clone());
            }
        }

        let health_checker = HealthChecker {
            config: HealthCheckConfig {
                interval: config.load_balancing.health_check_interval,
                timeout: config.load_balancing.health_check_timeout,
                unhealthy_threshold: config.load_balancing.unhealthy_threshold,
                healthy_threshold: config.load_balancing.healthy_threshold,
                check_endpoints: vec!["/health".to_string(), "/status".to_string()],
            },
            active_regions: region_health.clone(),
        };

        let dns_manager = DNSManager {
            config: DNSConfig {
                provider: DNSProvider::Route53, // Default to Route53
                ttl_seconds: config.load_balancing.dns_ttl_seconds,
                failover_enabled: true,
                geo_routing_enabled: config.load_balancing.geo_routing.enabled,
            },
            current_records: HashMap::new(),
        };

        Self {
            config,
            region_health,
            routing_table,
            health_checker,
            dns_manager,
        }
    }

    /// Start the load balancer background tasks
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting global load balancer");

        // Start health checking
        self.start_health_monitoring().await?;

        // Initialize DNS records
        self.initialize_dns_records().await?;

        info!("Global load balancer started successfully");
        Ok(())
    }

    /// Route a request to the optimal region
    pub async fn route_request(&self, request_context: &RequestContext) -> Result<RoutingDecision> {
        debug!("Routing request from country: {}", request_context.country);

        // Try geographic routing first
        if self.config.load_balancing.geo_routing.enabled {
            if let Some(decision) = self.route_geographically(request_context).await? {
                return Ok(decision);
            }
        }

        // Fall back to health-based routing
        self.route_by_health(request_context).await
    }

    /// Geographic routing based on client location
    async fn route_geographically(&self, context: &RequestContext) -> Result<Option<RoutingDecision>> {
        if let Some(region_codes) = self.routing_table.get(&context.country) {
            for region_code in region_codes {
                if let Some(region_health) = self.region_health.get(region_code) {
                    if matches!(region_health.status, HealthStatus::Healthy | HealthStatus::Degraded) {
                        if let Some(region_config) = self.config.get_region_by_code(region_code) {
                            return Ok(Some(RoutingDecision {
                                selected_region: region_code.clone(),
                                endpoint: region_config.endpoint.clone(),
                                routing_reason: RoutingReason::Geographic,
                                estimated_latency_ms: region_health.response_time_ms,
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Health-based routing to the healthiest available region
    async fn route_by_health(&self, _context: &RequestContext) -> Result<RoutingDecision> {
        let mut best_region = None;
        let mut best_score = f64::MIN;

        for (region_code, region_health) in &self.region_health {
            if matches!(region_health.status, HealthStatus::Healthy | HealthStatus::Degraded) {
                // Calculate routing score based on health metrics
                let score = self.calculate_routing_score(region_health);
                
                if score > best_score {
                    best_score = score;
                    best_region = Some(region_code);
                }
            }
        }

        if let Some(region_code) = best_region {
            if let Some(region_config) = self.config.get_region_by_code(region_code) {
                let reason = if best_score > 0.8 {
                    RoutingReason::HealthBased
                } else {
                    RoutingReason::LoadBalancing
                };

                return Ok(RoutingDecision {
                    selected_region: region_code.clone(),
                    endpoint: region_config.endpoint.clone(),
                    routing_reason: reason,
                    estimated_latency_ms: self.region_health[region_code].response_time_ms,
                });
            }
        }

        // Fall back to primary region
        if let Some(primary_region) = self.config.get_primary_region_config() {
            Ok(RoutingDecision {
                selected_region: primary_region.region_code.clone(),
                endpoint: primary_region.endpoint.clone(),
                routing_reason: RoutingReason::Default,
                estimated_latency_ms: 0,
            })
        } else {
            Err(anyhow::anyhow!("No healthy regions available and no primary region configured"))
        }
    }

    /// Calculate routing score for a region based on health metrics
    fn calculate_routing_score(&self, health: &RegionHealth) -> f64 {
        let mut score = 0.0;

        // Success rate weight: 40%
        score += health.success_rate * 0.4;

        // Error rate penalty: -30%
        score -= health.error_rate * 0.3;

        // Response time penalty: -20% (normalized to 1000ms)
        let response_time_penalty = (health.response_time_ms as f64 / 1000.0).min(1.0) * 0.2;
        score -= response_time_penalty;

        // Load penalty: -10% (normalized to max capacity)
        let load_penalty = (health.requests_per_second as f64 / 10000.0).min(1.0) * 0.1;
        score -= load_penalty;

        // Consecutive successes bonus
        let success_bonus = (health.consecutive_successes as f64 / 10.0).min(1.0) * 0.1;
        score += success_bonus;

        // Consecutive failures penalty
        let failure_penalty = (health.consecutive_failures as f64 / 5.0).min(1.0) * 0.2;
        score -= failure_penalty;

        score.max(0.0).min(1.0)
    }

    /// Start continuous health monitoring
    async fn start_health_monitoring(&mut self) -> Result<()> {
        let interval = self.config.load_balancing.health_check_interval;
        let regions = self.config.regions.clone();

        tokio::spawn(async move {
            let mut last_checks = HashMap::new();

            loop {
                for region in &regions {
                    let now = Instant::now();
                    
                    // Rate limit health checks
                    if let Some(last_check) = last_checks.get(&region.region_code) {
                        if now.duration_since(*last_check) < interval {
                            continue;
                        }
                    }

                    // Perform health check
                    match Self::check_region_health(region).await {
                        Ok(health) => {
                            debug!("Health check passed for region: {}", region.region_code);
                            last_checks.insert(region.region_code.clone(), now);
                        }
                        Err(e) => {
                            warn!("Health check failed for region {}: {}", region.region_code, e);
                        }
                    }
                }

                sleep(interval).await;
            }
        });

        Ok(())
    }

    /// Check health of a specific region
    async fn check_region_health(region: &RegionConfig) -> Result<RegionHealth> {
        let start_time = Instant::now();
        
        // Make HTTP request to health endpoint
        let client = reqwest::Client::new();
        let health_url = format!("{}/health", region.endpoint);
        
        let response = client
            .get(&health_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        let response_time = start_time.elapsed().as_millis() as u64;

        match response {
            Ok(resp) if resp.status().is_success() => {
                Ok(RegionHealth {
                    region_code: region.region_code.clone(),
                    status: HealthStatus::Healthy,
                    response_time_ms: response_time,
                    success_rate: 1.0,
                    requests_per_second: 0,
                    error_rate: 0.0,
                    last_health_check: Instant::now(),
                    consecutive_failures: 0,
                    consecutive_successes: 1,
                })
            }
            Ok(_) => {
                Ok(RegionHealth {
                    region_code: region.region_code.clone(),
                    status: HealthStatus::Degraded,
                    response_time_ms: response_time,
                    success_rate: 0.5,
                    requests_per_second: 0,
                    error_rate: 0.5,
                    last_health_check: Instant::now(),
                    consecutive_failures: 1,
                    consecutive_successes: 0,
                })
            }
            Err(_) => {
                Ok(RegionHealth {
                    region_code: region.region_code.clone(),
                    status: HealthStatus::Unhealthy,
                    response_time_ms: response_time,
                    success_rate: 0.0,
                    requests_per_second: 0,
                    error_rate: 1.0,
                    last_health_check: Instant::now(),
                    consecutive_failures: 1,
                    consecutive_successes: 0,
                })
            }
        }
    }

    /// Initialize DNS records for global routing
    async fn initialize_dns_records(&mut self) -> Result<()> {
        info!("Initializing DNS records for global routing");

        let active_regions = self.get_active_regions();
        
        // Create primary DNS record
        let primary_record = DNSRecord {
            name: "api.aframp.com".to_string(),
            values: active_regions.iter().map(|r| r.endpoint.clone()).collect(),
            ttl: self.config.load_balancing.dns_ttl_seconds,
            geo_routing: self.config.load_balancing.geo_routing.country_routing.clone(),
        };

        self.dns_manager.current_records.insert("api.aframp.com".to_string(), primary_record);

        // Update DNS provider
        self.update_dns_records().await?;

        info!("DNS records initialized successfully");
        Ok(())
    }

    /// Update DNS records with current region health
    async fn update_dns_records(&mut self) -> Result<()> {
        let active_regions = self.get_active_regions();
        
        for (name, record) in &mut self.dns_manager.current_records {
            // Update with current healthy endpoints
            record.values = active_regions.iter()
                .filter(|r| matches!(self.region_health[&r.region_code].status, HealthStatus::Healthy))
                .map(|r| r.endpoint.clone())
                .collect();

            // TODO: Actually update DNS provider
            debug!("Updated DNS record {} with {} values", name, record.values.len());
        }

        Ok(())
    }

    /// Get currently active and healthy regions
    fn get_active_regions(&self) -> Vec<&RegionConfig> {
        self.config.regions.iter()
            .filter(|r| {
                matches!(r.status, RegionStatus::Active) &&
                matches!(self.region_health[&r.region_code].status, HealthStatus::Healthy | HealthStatus::Degraded)
            })
            .collect()
    }

    /// Handle region failure
    pub async fn handle_region_failure(&mut self, region_code: &str) -> Result<()> {
        warn!("Handling region failure for: {}", region_code);

        // Update region health status
        if let Some(health) = self.region_health.get_mut(region_code) {
            health.status = HealthStatus::Unhealthy;
            health.consecutive_failures += 1;
            health.consecutive_successes = 0;
        }

        // Update DNS records to exclude failed region
        self.update_dns_records().await?;

        // Send notifications
        self.send_failure_notification(region_code).await?;

        error!("Region {} marked as failed and removed from routing", region_code);
        Ok(())
    }

    /// Handle region recovery
    pub async fn handle_region_recovery(&mut self, region_code: &str) -> Result<()> {
        info!("Handling region recovery for: {}", region_code);

        // Update region health status
        if let Some(health) = self.region_health.get_mut(region_code) {
            health.status = HealthStatus::Healthy;
            health.consecutive_failures = 0;
            health.consecutive_successes += 1;
        }

        // Update DNS records to include recovered region
        self.update_dns_records().await?;

        // Send notifications
        self.send_recovery_notification(region_code).await?;

        info!("Region {} marked as recovered and added back to routing", region_code);
        Ok(())
    }

    /// Get current load balancer status
    pub fn get_status(&self) -> LoadBalancerStatus {
        let healthy_regions = self.region_health.values()
            .filter(|h| matches!(h.status, HealthStatus::Healthy))
            .count();

        let degraded_regions = self.region_health.values()
            .filter(|h| matches!(h.status, HealthStatus::Degraded))
            .count();

        let unhealthy_regions = self.region_health.values()
            .filter(|h| matches!(h.status, HealthStatus::Unhealthy))
            .count();

        let total_rps = self.region_health.values()
            .map(|h| h.requests_per_second)
            .sum();

        let avg_response_time = if self.region_health.is_empty() {
            0
        } else {
            self.region_health.values()
                .map(|h| h.response_time_ms)
                .sum::<u64>() / self.region_health.len() as u64
        };

        LoadBalancerStatus {
            strategy: self.config.load_balancing.strategy.clone(),
            healthy_regions: healthy_regions as u32,
            degraded_regions: degraded_regions as u32,
            unhealthy_regions: unhealthy_regions as u32,
            total_requests_per_second: total_rps,
            average_response_time_ms: avg_response_time,
        }
    }

    /// Send failure notification
    async fn send_failure_notification(&self, region_code: &str) -> Result<()> {
        // TODO: Implement notification sending
        warn!("Region failure notification sent for: {}", region_code);
        Ok(())
    }

    /// Send recovery notification
    async fn send_recovery_notification(&self, region_code: &str) -> Result<()> {
        // TODO: Implement notification sending
        info!("Region recovery notification sent for: {}", region_code);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub client_ip: String,
    pub country: String,
    pub user_agent: String,
    pub request_path: String,
    pub request_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerStatus {
    pub strategy: super::LoadBalancingStrategy,
    pub healthy_regions: u32,
    pub degraded_regions: u32,
    pub unhealthy_regions: u32,
    pub total_requests_per_second: u32,
    pub average_response_time_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_score_calculation() {
        let load_balancer = GlobalLoadBalancer::new(MultiRegionConfig::default());
        
        let healthy_region = RegionHealth {
            region_code: "test".to_string(),
            status: HealthStatus::Healthy,
            response_time_ms: 100,
            success_rate: 0.99,
            requests_per_second: 1000,
            error_rate: 0.01,
            last_health_check: Instant::now(),
            consecutive_failures: 0,
            consecutive_successes: 5,
        };

        let score = load_balancer.calculate_routing_score(&healthy_region);
        assert!(score > 0.8);
    }

    #[test]
    fn test_unhealthy_region_score() {
        let load_balancer = GlobalLoadBalancer::new(MultiRegionConfig::default());
        
        let unhealthy_region = RegionHealth {
            region_code: "test".to_string(),
            status: HealthStatus::Unhealthy,
            response_time_ms: 5000,
            success_rate: 0.0,
            requests_per_second: 0,
            error_rate: 1.0,
            last_health_check: Instant::now(),
            consecutive_failures: 5,
            consecutive_successes: 0,
        };

        let score = load_balancer.calculate_routing_score(&unhealthy_region);
        assert!(score < 0.2);
    }

    #[tokio::test]
    async fn test_geographic_routing() {
        let mut load_balancer = GlobalLoadBalancer::new(MultiRegionConfig::default());
        
        let context = RequestContext {
            client_ip: "192.168.1.1".to_string(),
            country: "US".to_string(),
            user_agent: "test".to_string(),
            request_path: "/api/test".to_string(),
            request_method: "GET".to_string(),
        };

        // Mock healthy region
        load_balancer.region_health.insert("us-east-1".to_string(), RegionHealth {
            region_code: "us-east-1".to_string(),
            status: HealthStatus::Healthy,
            response_time_ms: 50,
            success_rate: 1.0,
            requests_per_second: 500,
            error_rate: 0.0,
            last_health_check: Instant::now(),
            consecutive_failures: 0,
            consecutive_successes: 10,
        });

        let decision = load_balancer.route_request(&context).await.unwrap();
        assert_eq!(decision.selected_region, "us-east-1");
        assert_eq!(decision.routing_reason, RoutingReason::Geographic);
    }
}
