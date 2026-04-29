//! Integration Tests for Multi-Region Deployment
//!
//! Tests the complete multi-region architecture including:
//! - Load balancing and geographic routing
//! - Database replication and failover
//! - Infrastructure as Code portability
//! - Global monitoring and observability

use aframp_backend::multi_region::{
    MultiRegionConfig, RegionConfig, RegionStatus, LoadBalancingStrategy,
    GlobalLoadBalancer, RequestContext, RoutingReason, DatabaseConfig,
    ReplicationMode, FailoverMode, ConsistencyLevel
};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn test_multi_region_config_default() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    
    // Verify default configuration
    assert_eq!(config.primary_region, "us-east-1");
    assert_eq!(config.regions.len(), 3);
    
    // Verify all regions have required fields
    for region in &config.regions {
        assert!(!region.name.is_empty());
        assert!(!region.region_code.is_empty());
        assert!(!region.endpoint.is_empty());
        assert!(!region.database_url.is_empty());
        assert!(!region.redis_url.is_empty());
        assert!(region.priority > 0);
        assert!(region.capacity.max_connections > 0);
        assert!(region.capacity.cpu_cores > 0);
        assert!(region.capacity.memory_gb > 0);
        assert!(region.capacity.storage_gb > 0);
    }
    
    // Verify database configuration
    assert!(matches!(config.database.replication_mode, ReplicationMode::Asynchronous));
    assert!(matches!(config.database.failover_mode, FailoverMode::Automatic));
    assert!(config.database.replication_lag_target_ms > 0);
    
    // Verify load balancing configuration
    assert!(matches!(config.load_balancing.strategy, LoadBalancingStrategy::Geographic));
    assert!(config.load_balancing.geo_routing.enabled);
    assert!(config.load_balancing.dns_ttl_seconds > 0);

    Ok(())
}

#[tokio::test]
async fn test_multi_region_config_validation() -> Result<(), anyhow::Error> {
    let mut config = MultiRegionConfig::default();
    
    // Valid configuration should pass
    assert!(config.validate().is_ok());
    
    // Test invalid primary region
    let original_primary = config.primary_region.clone();
    config.primary_region = "invalid-region".to_string();
    assert!(config.validate().is_err());
    config.primary_region = original_primary;
    
    // Test no active regions
    let original_statuses: Vec<RegionStatus> = config.regions.iter().map(|r| r.status.clone()).collect();
    for region in &mut config.regions {
        region.status = RegionStatus::Failed;
    }
    assert!(config.validate().is_err());
    
    // Restore original statuses
    for (region, status) in config.regions.iter_mut().zip(original_statuses) {
        region.status = status;
    }
    
    // Test geographic routing without country mapping
    let original_enabled = config.load_balancing.geo_routing.enabled;
    config.load_balancing.geo_routing.enabled = true;
    config.load_balancing.geo_routing.country_routing.clear();
    assert!(config.validate().is_err());
    config.load_balancing.geo_routing.country_routing.insert("US".to_string(), "us-east-1".to_string());
    assert!(config.validate().is_ok());
    config.load_balancing.geo_routing.enabled = original_enabled;

    Ok(())
}

#[tokio::test]
async fn test_region_lookup_and_routing() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    
    // Test region lookup by code
    assert!(config.get_region_by_code("us-east-1").is_some());
    assert!(config.get_region_by_code("eu-west-1").is_some());
    assert!(config.get_region_by_code("af-south-1").is_some());
    assert!(config.get_region_by_code("invalid-region").is_none());
    
    // Test primary region lookup
    let primary = config.get_primary_region_config();
    assert!(primary.is_some());
    assert_eq!(primary.unwrap().region_code, "us-east-1");
    
    // Test active regions
    let active_regions = config.get_active_regions();
    assert!(!active_regions.is_empty());
    assert!(active_regions.iter().all(|r| matches!(r.status, RegionStatus::Active)));
    
    // Test geographic routing
    assert!(config.get_region_for_country("US").is_some());
    assert_eq!(config.get_region_for_country("US").unwrap().region_code, "us-east-1");
    assert!(config.get_region_for_country("GB").is_some());
    assert_eq!(config.get_region_for_country("GB").unwrap().region_code, "eu-west-1");
    assert!(config.get_region_for_country("NG").is_some());
    assert_eq!(config.get_region_for_country("NG").unwrap().region_code, "af-south-1");
    
    // Test fallback to primary for unmapped countries
    assert!(config.get_region_for_country("XX").is_some());
    assert_eq!(config.get_region_for_country("XX").unwrap().region_code, "us-east-1");

    Ok(())
}

#[tokio::test]
async fn test_capacity_calculation() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    
    let total_capacity = config.calculate_total_capacity();
    
    // Verify total capacity is sum of all regions
    assert!(total_capacity.max_connections > 0);
    assert!(total_capacity.max_requests_per_second > 0);
    assert!(total_capacity.cpu_cores > 0);
    assert!(total_capacity.memory_gb > 0);
    assert!(total_capacity.storage_gb > 0);
    
    // Verify calculation is correct
    let expected_connections: u32 = config.regions.iter().map(|r| r.capacity.max_connections).sum();
    assert_eq!(total_capacity.max_connections, expected_connections);
    
    let expected_cpu: u32 = config.regions.iter().map(|r| r.capacity.cpu_cores).sum();
    assert_eq!(total_capacity.cpu_cores, expected_cpu);

    Ok(())
}

#[tokio::test]
async fn test_compliance_requirements() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    
    let compliance_summary = config.get_compliance_summary();
    
    // Verify all regions have compliance entries
    assert_eq!(compliance_summary.len(), config.regions.len());
    
    // Verify EU region has GDPR requirements
    if let Some(eu_requirements) = compliance_summary.get("eu-west-1") {
        assert!(eu_requirements.contains(&"GDPR".to_string()));
        assert!(eu_requirements.contains(&"Data Residency".to_string()));
        assert!(eu_requirements.contains(&"Local Storage".to_string()));
    }
    
    // Verify all regions have encryption at rest
    for (region, requirements) in &compliance_summary {
        assert!(requirements.contains(&"Encryption at Rest".to_string()), 
            "Region {} should require encryption at rest", region);
    }

    Ok(())
}

#[tokio::test]
async fn test_failover_time_estimation() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    
    let estimated_time = config.estimate_failover_time();
    
    // Verify failover time is reasonable
    assert!(estimated_time >= Duration::from_secs(60)); // At least failover timeout
    assert!(estimated_time <= Duration::from_secs(300)); // Shouldn't be more than 5 minutes
    
    // Verify components are included in estimation
    let base_time = config.failover.failover_timeout;
    let health_check_time = config.failover.health_check_interval;
    let dns_time = Duration::from_secs(config.load_balancing.dns_ttl_seconds);
    
    assert!(estimated_time >= base_time);
    assert!(estimated_time >= health_check_time);
    assert!(estimated_time >= dns_time);

    Ok(())
}

#[tokio::test]
async fn test_database_configuration() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    
    // Test database configuration
    assert!(!config.database.primary_region.is_empty());
    assert!(!config.database.replica_regions.is_empty());
    assert!(config.database.replication_lag_target_ms > 0);
    
    // Test backup strategy
    assert!(config.database.backup_strategy.continuous_backup);
    assert!(config.database.backup_strategy.point_in_time_recovery);
    assert!(config.database.backup_strategy.cross_region_backup);
    assert!(config.database.backup_strategy.backup_retention_days > 0);
    assert!(config.database.backup_strategy.backup_frequency_hours > 0);
    
    // Test consistency levels
    match config.database.consistency_level {
        ConsistencyLevel::Strong => {}, // Valid
        ConsistencyLevel::Eventual => {}, // Valid
        ConsistencyLevel::BoundedStaleness(duration) => {
            assert!(duration > Duration::from_secs(0));
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_load_balancer_initialization() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    let mut load_balancer = GlobalLoadBalancer::new(config);
    
    // Test load balancer status before start
    let status = load_balancer.get_status();
    assert_eq!(status.healthy_regions, 0); // No health checks yet
    assert_eq!(status.degraded_regions, 0);
    assert_eq!(status.unhealthy_regions, 0);
    
    // Test that we can create routing decisions (will use fallback logic)
    let context = RequestContext {
        client_ip: "192.168.1.100".to_string(),
        country: "US".to_string(),
        user_agent: "Test Browser".to_string(),
        request_path: "/api/test".to_string(),
        request_method: "GET".to_string(),
    };
    
    // This should succeed even without health checks
    let decision = load_balancer.route_request(&context).await?;
    assert!(!decision.selected_region.is_empty());
    assert!(!decision.endpoint.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_geographic_routing() -> Result<(), anyhow::Error> {
    let mut config = MultiRegionConfig::default();
    let mut load_balancer = GlobalLoadBalancer::new(config);
    
    // Test routing for different countries
    let test_cases = vec![
        ("US", "us-east-1"),
        ("CA", "us-east-1"),
        ("GB", "eu-west-1"),
        ("DE", "eu-west-1"),
        ("FR", "eu-west-1"),
        ("NG", "af-south-1"),
        ("ZA", "af-south-1"),
        ("KE", "af-south-1"),
        ("XX", "us-east-1"), // Fallback to primary
    ];
    
    for (country, expected_region) in test_cases {
        let context = RequestContext {
            client_ip: "192.168.1.100".to_string(),
            country: country.to_string(),
            user_agent: "Test Browser".to_string(),
            request_path: "/api/test".to_string(),
            request_method: "GET".to_string(),
        };
        
        // Mock healthy regions for testing
        load_balancer.region_health.insert(expected_region.to_string(), 
            aframp_backend::multi_region::RegionHealth {
                region_code: expected_region.to_string(),
                status: aframp_backend::multi_region::HealthStatus::Healthy,
                response_time_ms: 50,
                success_rate: 1.0,
                requests_per_second: 100,
                error_rate: 0.0,
                last_health_check: std::time::Instant::now(),
                consecutive_failures: 0,
                consecutive_successes: 10,
            });
        
        let decision = load_balancer.route_request(&context).await?;
        
        if country != "XX" {
            assert_eq!(decision.selected_region, expected_region, 
                "Country {} should route to {}", country, expected_region);
            assert_eq!(decision.routing_reason, RoutingReason::Geographic);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_health_based_routing() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    let mut load_balancer = GlobalLoadBalancer::new(config);
    
    // Mock regions with different health statuses
    load_balancer.region_health.insert("us-east-1".to_string(), 
        aframp_backend::multi_region::RegionHealth {
            region_code: "us-east-1".to_string(),
            status: aframp_backend::multi_region::HealthStatus::Healthy,
            response_time_ms: 50,
            success_rate: 1.0,
            requests_per_second: 100,
            error_rate: 0.0,
            last_health_check: std::time::Instant::now(),
            consecutive_failures: 0,
            consecutive_successes: 10,
        });
    
    load_balancer.region_health.insert("eu-west-1".to_string(), 
        aframp_backend::multi_region::RegionHealth {
            region_code: "eu-west-1".to_string(),
            status: aframp_backend::multi_region::HealthStatus::Degraded,
            response_time_ms: 200,
            success_rate: 0.8,
            requests_per_second: 50,
            error_rate: 0.2,
            last_health_check: std::time::Instant::now(),
            consecutive_failures: 2,
            consecutive_successes: 0,
        });
    
    load_balancer.region_health.insert("af-south-1".to_string(), 
        aframp_backend::multi_region::RegionHealth {
            region_code: "af-south-1".to_string(),
            status: aframp_backend::multi_region::HealthStatus::Unhealthy,
            response_time_ms: 5000,
            success_rate: 0.0,
            requests_per_second: 0,
            error_rate: 1.0,
            last_health_check: std::time::Instant::now(),
            consecutive_failures: 5,
            consecutive_successes: 0,
        });
    
    let context = RequestContext {
        client_ip: "192.168.1.100".to_string(),
        country: "XX".to_string(), // Unmapped country to force health-based routing
        user_agent: "Test Browser".to_string(),
        request_path: "/api/test".to_string(),
        request_method: "GET".to_string(),
    };
    
    let decision = load_balancer.route_request(&context).await?;
    
    // Should route to healthiest region (us-east-1)
    assert_eq!(decision.selected_region, "us-east-1");
    assert_eq!(decision.routing_reason, RoutingReason::HealthBased);

    Ok(())
}

#[tokio::test]
async fn test_routing_score_calculation() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    let load_balancer = GlobalLoadBalancer::new(config);
    
    // Test healthy region score
    let healthy_region = aframp_backend::multi_region::RegionHealth {
        region_code: "test".to_string(),
        status: aframp_backend::multi_region::HealthStatus::Healthy,
        response_time_ms: 50,
        success_rate: 1.0,
        requests_per_second: 100,
        error_rate: 0.0,
        last_health_check: std::time::Instant::now(),
        consecutive_failures: 0,
        consecutive_successes: 10,
    };
    
    let healthy_score = load_balancer.calculate_routing_score(&healthy_region);
    assert!(healthy_score > 0.8);
    
    // Test degraded region score
    let degraded_region = aframp_backend::multi_region::RegionHealth {
        region_code: "test".to_string(),
        status: aframp_backend::multi_region::HealthStatus::Degraded,
        response_time_ms: 200,
        success_rate: 0.8,
        requests_per_second: 50,
        error_rate: 0.2,
        last_health_check: std::time::Instant::now(),
        consecutive_failures: 2,
        consecutive_successes: 0,
    };
    
    let degraded_score = load_balancer.calculate_routing_score(&degraded_region);
    assert!(degraded_score < healthy_score);
    assert!(degraded_score > 0.3);
    
    // Test unhealthy region score
    let unhealthy_region = aframp_backend::multi_region::RegionHealth {
        region_code: "test".to_string(),
        status: aframp_backend::multi_region::HealthStatus::Unhealthy,
        response_time_ms: 5000,
        success_rate: 0.0,
        requests_per_second: 0,
        error_rate: 1.0,
        last_health_check: std::time::Instant::now(),
        consecutive_failures: 5,
        consecutive_successes: 0,
    };
    
    let unhealthy_score = load_balancer.calculate_routing_score(&unhealthy_region);
    assert!(unhealthy_score < 0.2);

    Ok(())
}

#[tokio::test]
async fn test_region_failure_handling() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    let mut load_balancer = GlobalLoadBalancer::new(config);
    
    // Set up initial healthy state
    load_balancer.region_health.insert("us-east-1".to_string(), 
        aframp_backend::multi_region::RegionHealth {
            region_code: "us-east-1".to_string(),
            status: aframp_backend::multi_region::HealthStatus::Healthy,
            response_time_ms: 50,
            success_rate: 1.0,
            requests_per_second: 100,
            error_rate: 0.0,
            last_health_check: std::time::Instant::now(),
            consecutive_failures: 0,
            consecutive_successes: 10,
        });
    
    // Handle region failure
    load_balancer.handle_region_failure("us-east-1").await?;
    
    // Verify region is marked as failed
    if let Some(health) = load_balancer.region_health.get("us-east-1") {
        assert!(matches!(health.status, aframp_backend::multi_region::HealthStatus::Unhealthy));
        assert!(health.consecutive_failures > 0);
        assert_eq!(health.consecutive_successes, 0);
    }
    
    // Handle region recovery
    load_balancer.handle_region_recovery("us-east-1").await?;
    
    // Verify region is marked as healthy
    if let Some(health) = load_balancer.region_health.get("us-east-1") {
        assert!(matches!(health.status, aframp_backend::multi_region::HealthStatus::Healthy));
        assert_eq!(health.consecutive_failures, 0);
        assert!(health.consecutive_successes > 0);
    }

    Ok(())
}

#[tokio::test]
async fn test_load_balancer_status() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    let mut load_balancer = GlobalLoadBalancer::new(config);
    
    // Set up mixed health states
    load_balancer.region_health.insert("us-east-1".to_string(), 
        aframp_backend::multi_region::RegionHealth {
            region_code: "us-east-1".to_string(),
            status: aframp_backend::multi_region::HealthStatus::Healthy,
            response_time_ms: 50,
            success_rate: 1.0,
            requests_per_second: 100,
            error_rate: 0.0,
            last_health_check: std::time::Instant::now(),
            consecutive_failures: 0,
            consecutive_successes: 10,
        });
    
    load_balancer.region_health.insert("eu-west-1".to_string(), 
        aframp_backend::multi_region::RegionHealth {
            region_code: "eu-west-1".to_string(),
            status: aframp_backend::multi_region::HealthStatus::Degraded,
            response_time_ms: 200,
            success_rate: 0.8,
            requests_per_second: 50,
            error_rate: 0.2,
            last_health_check: std::time::Instant::now(),
            consecutive_failures: 2,
            consecutive_successes: 0,
        });
    
    load_balancer.region_health.insert("af-south-1".to_string(), 
        aframp_backend::multi_region::RegionHealth {
            region_code: "af-south-1".to_string(),
            status: aframp_backend::multi_region::HealthStatus::Unhealthy,
            response_time_ms: 5000,
            success_rate: 0.0,
            requests_per_second: 0,
            error_rate: 1.0,
            last_health_check: std::time::Instant::now(),
            consecutive_failures: 5,
            consecutive_successes: 0,
        });
    
    let status = load_balancer.get_status();
    
    assert_eq!(status.healthy_regions, 1);
    assert_eq!(status.degraded_regions, 1);
    assert_eq!(status.unhealthy_regions, 1);
    assert_eq!(status.total_requests_per_second, 150); // 100 + 50 + 0
    assert_eq!(status.average_response_time_ms, (50 + 200 + 5000) / 3);

    Ok(())
}

#[tokio::test]
async fn test_configuration_from_environment() -> Result<(), anyhow::Error> {
    // Set environment variables
    std::env::set_var("PRIMARY_REGION", "eu-west-1");
    std::env::set_var("DATABASE_REPLICATION_LAG_MS", "300");
    std::env::set_var("FAILOVER_TIMEOUT_SECONDS", "120");
    std::env::set_var("AUTO_FAILOVER_ENABLED", "false");
    
    let config = MultiRegionConfig::from_env()?;
    
    // Verify environment overrides
    assert_eq!(config.primary_region, "eu-west-1");
    assert_eq!(config.database.replication_lag_target_ms, 300);
    assert_eq!(config.failover.failover_timeout, Duration::from_secs(120));
    assert!(!config.failover.automatic_failover);
    
    // Clean up
    std::env::remove_var("PRIMARY_REGION");
    std::env::remove_var("DATABASE_REPLICATION_LAG_MS");
    std::env::remove_var("FAILOVER_TIMEOUT_SECONDS");
    std::env::remove_var("AUTO_FAILOVER_ENABLED");

    Ok(())
}

#[tokio::test]
async fn test_infrastructure_configuration() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    
    // Test infrastructure configuration
    assert!(config.infrastructure.as_code);
    assert!(config.infrastructure.terraform_enabled);
    assert!(!config.infrastructure.pulumi_enabled);
    assert!(config.infrastructure.environment_isolation);
    
    // Test secret management
    assert!(matches!(config.infrastructure.secret_management.provider, 
        aframp_backend::multi_region::SecretProvider::AwsSecretsManager));
    assert!(config.infrastructure.secret_management.cross_region_sync);
    assert!(config.infrastructure.secret_management.rotation_enabled);
    assert!(config.infrastructure.secret_management.rotation_period_days > 0);

    Ok(())
}

#[tokio::test]
async fn test_monitoring_configuration() -> Result<(), anyhow::Error> {
    let config = MultiRegionConfig::default();
    
    // Test monitoring configuration
    assert!(config.monitoring.centralized_logging);
    assert!(config.monitoring.metrics_aggregation);
    assert!(config.monitoring.regional_alerts);
    assert!(config.monitoring.global_alerts);
    assert!(!config.monitoring.dashboard_regions.is_empty());
    assert!(config.monitoring.retention_period_days > 0);
    
    // Test alert routing
    assert!(!config.monitoring.alert_routing.severity_routing.is_empty());
    assert!(!config.monitoring.alert_routing.escalation_rules.is_empty());
    
    // Verify escalation rules have required fields
    for rule in &config.monitoring.alert_routing.escalation_rules {
        assert!(!rule.alert_type.is_empty());
        assert!(rule.threshold_duration > Duration::from_secs(0));
        assert!(rule.escalation_level > 0);
        assert!(!rule.notification_channels.is_empty());
    }

    Ok(())
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn benchmark_routing_decisions() -> Result<(), anyhow::Error> {
        let config = MultiRegionConfig::default();
        let mut load_balancer = GlobalLoadBalancer::new(config);
        
        // Set up healthy regions
        for region in &load_balancer.config.regions {
            load_balancer.region_health.insert(region.region_code.clone(), 
                aframp_backend::multi_region::RegionHealth {
                    region_code: region.region_code.clone(),
                    status: aframp_backend::multi_region::HealthStatus::Healthy,
                    response_time_ms: 50,
                    success_rate: 1.0,
                    requests_per_second: 100,
                    error_rate: 0.0,
                    last_health_check: std::time::Instant::now(),
                    consecutive_failures: 0,
                    consecutive_successes: 10,
                });
        }
        
        const NUM_REQUESTS: usize = 10000;
        let countries = vec!["US", "GB", "NG", "DE", "FR", "ZA", "KE", "CA", "XX"];
        
        let start = Instant::now();
        
        for i in 0..NUM_REQUESTS {
            let country = countries[i % countries.len()];
            let context = RequestContext {
                client_ip: format!("192.168.1.{}", i % 255),
                country: country.to_string(),
                user_agent: "Test Browser".to_string(),
                request_path: "/api/test".to_string(),
                request_method: "GET".to_string(),
            };
            
            let _decision = load_balancer.route_request(&context).await?;
        }
        
        let duration = start.elapsed();
        
        println!("Routing Decision Benchmark:");
        println!("{} requests routed in {:?} ({:.2} req/sec)", 
            NUM_REQUESTS, duration, NUM_REQUESTS as f64 / duration.as_secs_f64());
        
        // Should be able to route at least 1000 requests per second
        let req_per_sec = NUM_REQUESTS as f64 / duration.as_secs_f64();
        assert!(req_per_sec > 1000.0, "Routing too slow: {:.2} req/sec", req_per_sec);

        Ok(())
    }

    #[tokio::test]
    async fn benchmark_configuration_operations() -> Result<(), anyhow::Error> {
        let config = MultiRegionConfig::default();
        
        const NUM_OPERATIONS: usize = 1000;
        
        let start = Instant::now();
        
        for _i in 0..NUM_OPERATIONS {
            // Test various configuration operations
            let _primary = config.get_primary_region_config();
            let _active = config.get_active_regions();
            let _capacity = config.calculate_total_capacity();
            let _compliance = config.get_compliance_summary();
            let _failover_time = config.estimate_failover_time();
            
            // Test region lookups
            let _region = config.get_region_by_code("us-east-1");
            let _country_region = config.get_region_for_country("US");
        }
        
        let duration = start.elapsed();
        
        println!("Configuration Operations Benchmark:");
        println!("{} operations in {:?} ({:.2} ops/sec)", 
            NUM_OPERATIONS * 6, duration, (NUM_OPERATIONS * 6) as f64 / duration.as_secs_f64());

        Ok(())
    }
}
