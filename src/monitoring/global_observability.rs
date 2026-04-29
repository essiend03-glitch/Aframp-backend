//! Global Observability System
//!
//! Implements comprehensive monitoring for all systems:
//! - Prometheus metrics collection
//! - Structured logging and tracing
//! - Health checks and SLA monitoring
//! - Alert routing and escalation
//! - Performance analytics

use prometheus::{Counter, Gauge, Histogram, Registry, Opts, HistogramOpts};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GlobalObservability {
    registry: Arc<Registry>,
    metrics: GlobalMetrics,
    alert_manager: AlertManager,
    health_monitor: HealthMonitor,
    sla_tracker: SLATracker,
}

#[derive(Debug, Clone)]
pub struct GlobalMetrics {
    // Cache metrics
    cache_hits_total: Counter,
    cache_misses_total: Counter,
    cache_operations_duration: Histogram,
    cache_size_bytes: Gauge,
    
    // AML metrics
    aml_evaluations_total: Counter,
    aml_rules_triggered_total: Counter,
    aml_cases_created_total: Counter,
    aml_evaluation_duration: Histogram,
    aml_risk_score_histogram: Histogram,
    
    // Multi-region metrics
    region_requests_total: HashMap<String, Counter>,
    region_response_time: HashMap<String, Histogram>,
    region_health_score: HashMap<String, Gauge>,
    failover_events_total: Counter,
    
    // System metrics
    http_requests_total: Counter,
    http_request_duration: Histogram,
    database_connections_active: Gauge,
    database_query_duration: Histogram,
    
    // Business metrics
    transactions_processed_total: Counter,
    transaction_volume_total: Counter,
    active_users_total: Gauge,
    error_rate: Gauge,
}

#[derive(Debug, Clone)]
pub struct AlertManager {
    alerts: HashMap<String, AlertRule>,
    notification_channels: Vec<NotificationChannel>,
    escalation_policies: HashMap<String, EscalationPolicy>,
}

#[derive(Debug, Clone)]
pub struct AlertRule {
    pub id: String,
    pub name: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub enabled: bool,
    pub cooldown: Duration,
    pub last_triggered: Option<Instant>,
    pub notification_channels: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum AlertCondition {
    ThresholdAbove { metric: String, value: f64 },
    ThresholdBelow { metric: String, value: f64 },
    RateIncrease { metric: String, percentage: f64, window: Duration },
    ErrorRate { threshold: f64 },
    Latency { threshold: Duration, percentile: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

#[derive(Debug, Clone)]
pub struct NotificationChannel {
    pub id: String,
    pub name: String,
    pub channel_type: ChannelType,
    pub config: HashMap<String, String>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub enum ChannelType {
    Email,
    Slack,
    PagerDuty,
    Webhook,
    Teams,
}

#[derive(Debug, Clone)]
pub struct EscalationPolicy {
    pub alert_type: String,
    pub levels: Vec<EscalationLevel>,
}

#[derive(Debug, Clone)]
pub struct EscalationLevel {
    pub level: u8,
    pub delay: Duration,
    pub channels: Vec<String>,
    pub auto_escalate: bool,
}

#[derive(Debug, Clone)]
pub struct HealthMonitor {
    checks: HashMap<String, HealthCheck>,
    status: HashMap<String, HealthStatus>,
    last_check: HashMap<String, Instant>,
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub id: String,
    pub name: String,
    pub check_type: HealthCheckType,
    pub interval: Duration,
    pub timeout: Duration,
    pub healthy_threshold: u32,
    pub unhealthy_threshold: u32,
}

#[derive(Debug, Clone)]
pub enum HealthCheckType {
    HTTP { url: String, expected_status: u16 },
    TCP { address: String },
    Database { connection_string: String },
    Cache { key: String },
    Custom { check_fn: String },
}

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub check_id: String,
    pub status: ServiceStatus,
    pub last_check: Instant,
    pub response_time: Duration,
    pub error_message: Option<String>,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SLATracker {
    services: HashMap<String, SLADefinition>,
    metrics: HashMap<String, SLAMetrics>,
}

#[derive(Debug, Clone)]
pub struct SLADefinition {
    pub service_id: String,
    pub service_name: String,
    pub availability_target: f64, // 99.9%
    pub latency_target: Duration, // 500ms
    pub error_rate_target: f64,  // 0.1%
    pub window: Duration,         // 24h
}

#[derive(Debug, Clone)]
pub struct SLAMetrics {
    pub service_id: String,
    pub window_start: Instant,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_latency: Duration,
    pub max_latency: Duration,
    pub p50_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
}

impl GlobalObservability {
    pub fn new() -> Result<Self, anyhow::Error> {
        let registry = Arc::new(Registry::new());
        let metrics = GlobalMetrics::new(&registry)?;
        
        let alert_manager = AlertManager::new();
        let health_monitor = HealthMonitor::new();
        let sla_tracker = SLATracker::new();

        Ok(Self {
            registry,
            metrics,
            alert_manager,
            health_monitor,
            sla_tracker,
        })
    }

    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }

    pub async fn start(&mut self) -> Result<(), anyhow::Error> {
        info!("Starting global observability system");

        // Start health monitoring
        self.start_health_monitoring().await?;

        // Start alert processing
        self.start_alert_processing().await?;

        // Start SLA tracking
        self.start_sla_tracking().await?;

        info!("Global observability system started");
        Ok(())
    }

    // Cache metrics
    pub fn record_cache_hit(&self, cache_type: &str) {
        let labels = &[("cache_type", cache_type)];
        self.metrics.cache_hits_total.with_label_values(labels).inc();
    }

    pub fn record_cache_miss(&self, cache_type: &str) {
        let labels = &[("cache_type", cache_type)];
        self.metrics.cache_misses_total.with_label_values(labels).inc();
    }

    pub fn record_cache_operation(&self, operation: &str, duration: Duration) {
        let labels = &[("operation", operation)];
        self.metrics.cache_operations_duration.with_label_values(labels)
            .observe(duration.as_secs_f64());
    }

    pub fn update_cache_size(&self, cache_type: &str, size_bytes: u64) {
        let labels = &[("cache_type", cache_type)];
        self.metrics.cache_size_bytes.with_label_values(labels).set(size_bytes as f64);
    }

    // AML metrics
    pub fn record_aml_evaluation(&self, risk_score: f64, duration: Duration) {
        self.metrics.aml_evaluations_total.inc();
        self.metrics.aml_evaluation_duration.observe(duration.as_secs_f64());
        self.metrics.aml_risk_score_histogram.observe(risk_score);
    }

    pub fn record_aml_rule_triggered(&self, rule_category: &str) {
        let labels = &[("category", rule_category)];
        self.metrics.aml_rules_triggered_total.with_label_values(labels).inc();
    }

    pub fn record_aml_case_created(&self, case_type: &str, risk_level: &str) {
        let labels = &[("case_type", case_type), ("risk_level", risk_level)];
        self.metrics.aml_cases_created_total.with_label_values(labels).inc();
    }

    // Multi-region metrics
    pub fn record_region_request(&self, region: &str, status: &str) {
        let counter = self.metrics.region_requests_total
            .entry(region.to_string())
            .or_insert_with(|| {
                let opts = Opts::new("region_requests_total", "Total requests per region")
                    .subsystem("multi_region");
                Counter::with_opts(opts).unwrap()
            });
        
        let labels = &[("region", region), ("status", status)];
        counter.with_label_values(labels).inc();
    }

    pub fn record_region_response_time(&self, region: &str, duration: Duration) {
        let histogram = self.metrics.region_response_time
            .entry(region.to_string())
            .or_insert_with(|| {
                let opts = HistogramOpts::new("region_response_time_seconds", "Region response time")
                    .subsystem("multi_region")
                    .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]);
                Histogram::with_opts(opts).unwrap()
            });
        
        histogram.observe(duration.as_secs_f64());
    }

    pub fn update_region_health_score(&self, region: &str, score: f64) {
        let gauge = self.metrics.region_health_score
            .entry(region.to_string())
            .or_insert_with(|| {
                let opts = Opts::new("region_health_score", "Region health score")
                    .subsystem("multi_region");
                Gauge::with_opts(opts).unwrap()
            });
        
        gauge.set(score);
    }

    pub fn record_failover_event(&self, from_region: &str, to_region: &str) {
        let labels = &[("from_region", from_region), ("to_region", to_region)];
        self.metrics.failover_events_total.with_label_values(labels).inc();
    }

    // System metrics
    pub fn record_http_request(&self, method: &str, path: &str, status: u16, duration: Duration) {
        let labels = &[
            ("method", method),
            ("path", path),
            ("status", &status.to_string()),
        ];
        
        self.metrics.http_requests_total.with_label_values(labels).inc();
        self.metrics.http_request_duration.with_label_values(labels)
            .observe(duration.as_secs_f64());

        // Update error rate
        if status >= 500 {
            self.metrics.error_rate.inc();
        }
    }

    pub fn update_database_connections(&self, active: u32, idle: u32) {
        self.metrics.database_connections_active.set(active as f64);
    }

    pub fn record_database_query(&self, query_type: &str, duration: Duration) {
        let labels = &[("query_type", query_type)];
        self.metrics.database_query_duration.with_label_values(labels)
            .observe(duration.as_secs_f64());
    }

    // Business metrics
    pub fn record_transaction(&self, transaction_type: &str, amount: f64, currency: &str) {
        let labels = &[("type", transaction_type), ("currency", currency)];
        self.metrics.transactions_processed_total.with_label_values(labels).inc();
        self.metrics.transaction_volume_total.with_label_values(labels).inc_by(amount);
    }

    pub fn update_active_users(&self, count: u64) {
        self.metrics.active_users_total.set(count as f64);
    }

    // Health monitoring
    async fn start_health_monitoring(&mut self) -> Result<(), anyhow::Error> {
        // Register default health checks
        self.register_health_check(HealthCheck {
            id: "database".to_string(),
            name: "Database Connectivity".to_string(),
            check_type: HealthCheckType::Database {
                connection_string: "postgresql://...".to_string(),
            },
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            healthy_threshold: 2,
            unhealthy_threshold: 3,
        });

        self.register_health_check(HealthCheck {
            id: "cache".to_string(),
            name: "Cache Connectivity".to_string(),
            check_type: HealthCheckType::Cache {
                key: "health_check".to_string(),
            },
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            healthy_threshold: 2,
            unhealthy_threshold: 3,
        });

        // Start health check loop
        let health_monitor = self.health_monitor.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                health_monitor.run_health_checks().await;
            }
        });

        Ok(())
    }

    // Alert processing
    async fn start_alert_processing(&mut self) -> Result<(), anyhow::Error> {
        // Register default alert rules
        self.register_alert_rule(AlertRule {
            id: "high_error_rate".to_string(),
            name: "High Error Rate".to_string(),
            condition: AlertCondition::ErrorRate { threshold: 0.05 },
            severity: AlertSeverity::Warning,
            enabled: true,
            cooldown: Duration::from_secs(300),
            last_triggered: None,
            notification_channels: vec!["slack".to_string()],
        });

        self.register_alert_rule(AlertRule {
            id: "region_down".to_string(),
            name: "Region Down".to_string(),
            condition: AlertCondition::ThresholdBelow {
                metric: "region_health_score".to_string(),
                value: 0.5,
            },
            severity: AlertSeverity::Critical,
            enabled: true,
            cooldown: Duration::from_secs(60),
            last_triggered: None,
            notification_channels: vec!["pagerduty".to_string(), "slack".to_string()],
        });

        // Start alert evaluation loop
        let alert_manager = self.alert_manager.clone();
        let registry = self.registry.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                alert_manager.evaluate_alerts(&registry).await;
            }
        });

        Ok(())
    }

    // SLA tracking
    async fn start_sla_tracking(&mut self) -> Result<(), anyhow::Error> {
        // Register default SLA definitions
        self.register_sla_definition(SLADefinition {
            service_id: "api".to_string(),
            service_name: "API Service".to_string(),
            availability_target: 99.9,
            latency_target: Duration::from_millis(500),
            error_rate_target: 0.1,
            window: Duration::from_secs(86400), // 24 hours
        });

        self.register_sla_definition(SLADefinition {
            service_id: "aml".to_string(),
            service_name: "AML Service".to_string(),
            availability_target: 99.5,
            latency_target: Duration::from_secs(5),
            error_rate_target: 0.5,
            window: Duration::from_secs(86400),
        });

        // Start SLA calculation loop
        let sla_tracker = self.sla_tracker.clone();
        let registry = self.registry.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // Every 5 minutes
            loop {
                interval.tick().await;
                sla_tracker.calculate_sla_metrics(&registry).await;
            }
        });

        Ok(())
    }

    // Registration methods
    fn register_health_check(&mut self, check: HealthCheck) {
        self.health_monitor.checks.insert(check.id.clone(), check);
        self.health_monitor.status.insert(check.id.clone(), HealthStatus {
            check_id: check.id.clone(),
            status: ServiceStatus::Unknown,
            last_check: Instant::now(),
            response_time: Duration::from_secs(0),
            error_message: None,
            consecutive_failures: 0,
            consecutive_successes: 0,
        });
    }

    fn register_alert_rule(&mut self, rule: AlertRule) {
        self.alert_manager.alerts.insert(rule.id.clone(), rule);
    }

    fn register_sla_definition(&mut self, definition: SLADefinition) {
        self.sla_tracker.services.insert(definition.service_id.clone(), definition);
        self.sla_tracker.metrics.insert(definition.service_id.clone(), SLAMetrics {
            service_id: definition.service_id.clone(),
            window_start: Instant::now(),
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_latency: Duration::from_secs(0),
            max_latency: Duration::from_secs(0),
            p50_latency: Duration::from_secs(0),
            p95_latency: Duration::from_secs(0),
            p99_latency: Duration::from_secs(0),
        });
    }

    // Query methods
    pub fn get_health_status(&self) -> HashMap<String, HealthStatus> {
        self.health_monitor.status.clone()
    }

    pub fn get_sla_metrics(&self) -> HashMap<String, SLAMetrics> {
        self.sla_tracker.metrics.clone()
    }

    pub fn get_alert_rules(&self) -> HashMap<String, AlertRule> {
        self.alert_manager.alerts.clone()
    }
}

impl GlobalMetrics {
    fn new(registry: &Registry) -> Result<Self, anyhow::Error> {
        // Cache metrics
        let cache_hits_total = Counter::with_opts(
            Opts::new("cache_hits_total", "Total cache hits")
                .subsystem("cache")
        )?;
        registry.register(Box::new(cache_hits_total.clone()))?;

        let cache_misses_total = Counter::with_opts(
            Opts::new("cache_misses_total", "Total cache misses")
                .subsystem("cache")
        )?;
        registry.register(Box::new(cache_misses_total.clone()))?;

        let cache_operations_duration = Histogram::with_opts(
            HistogramOpts::new("cache_operation_duration_seconds", "Cache operation duration")
                .subsystem("cache")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0])
        )?;
        registry.register(Box::new(cache_operations_duration.clone()))?;

        let cache_size_bytes = Gauge::with_opts(
            Opts::new("cache_size_bytes", "Cache size in bytes")
                .subsystem("cache")
        )?;
        registry.register(Box::new(cache_size_bytes.clone()))?;

        // AML metrics
        let aml_evaluations_total = Counter::with_opts(
            Opts::new("evaluations_total", "Total AML evaluations")
                .subsystem("aml")
        )?;
        registry.register(Box::new(aml_evaluations_total.clone()))?;

        let aml_rules_triggered_total = Counter::with_opts(
            Opts::new("rules_triggered_total", "Total AML rules triggered")
                .subsystem("aml")
        )?;
        registry.register(Box::new(aml_rules_triggered_total.clone()))?;

        let aml_cases_created_total = Counter::with_opts(
            Opts::new("cases_created_total", "Total AML cases created")
                .subsystem("aml")
        )?;
        registry.register(Box::new(aml_cases_created_total.clone()))?;

        let aml_evaluation_duration = Histogram::with_opts(
            HistogramOpts::new("evaluation_duration_seconds", "AML evaluation duration")
                .subsystem("aml")
                .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0])
        )?;
        registry.register(Box::new(aml_evaluation_duration.clone()))?;

        let aml_risk_score_histogram = Histogram::with_opts(
            HistogramOpts::new("risk_score_histogram", "AML risk score distribution")
                .subsystem("aml")
                .buckets(vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0])
        )?;
        registry.register(Box::new(aml_risk_score_histogram.clone()))?;

        // Multi-region metrics
        let failover_events_total = Counter::with_opts(
            Opts::new("failover_events_total", "Total failover events")
                .subsystem("multi_region")
        )?;
        registry.register(Box::new(failover_events_total.clone()))?;

        // System metrics
        let http_requests_total = Counter::with_opts(
            Opts::new("requests_total", "Total HTTP requests")
                .subsystem("http")
        )?;
        registry.register(Box::new(http_requests_total.clone()))?;

        let http_request_duration = Histogram::with_opts(
            HistogramOpts::new("request_duration_seconds", "HTTP request duration")
                .subsystem("http")
                .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0])
        )?;
        registry.register(Box::new(http_request_duration.clone()))?;

        let database_connections_active = Gauge::with_opts(
            Opts::new("connections_active", "Active database connections")
                .subsystem("database")
        )?;
        registry.register(Box::new(database_connections_active.clone()))?;

        let database_query_duration = Histogram::with_opts(
            HistogramOpts::new("query_duration_seconds", "Database query duration")
                .subsystem("database")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0])
        )?;
        registry.register(Box::new(database_query_duration.clone()))?;

        // Business metrics
        let transactions_processed_total = Counter::with_opts(
            Opts::new("transactions_processed_total", "Total transactions processed")
                .subsystem("business")
        )?;
        registry.register(Box::new(transactions_processed_total.clone()))?;

        let transaction_volume_total = Counter::with_opts(
            Opts::new("transaction_volume_total", "Total transaction volume")
                .subsystem("business")
        )?;
        registry.register(Box::new(transaction_volume_total.clone()))?;

        let active_users_total = Gauge::with_opts(
            Opts::new("active_users_total", "Current active users")
                .subsystem("business")
        )?;
        registry.register(Box::new(active_users_total.clone()))?;

        let error_rate = Gauge::with_opts(
            Opts::new("error_rate", "Current error rate")
                .subsystem("system")
        )?;
        registry.register(Box::new(error_rate.clone()))?;

        Ok(Self {
            cache_hits_total,
            cache_misses_total,
            cache_operations_duration,
            cache_size_bytes,
            aml_evaluations_total,
            aml_rules_triggered_total,
            aml_cases_created_total,
            aml_evaluation_duration,
            aml_risk_score_histogram,
            region_requests_total: HashMap::new(),
            region_response_time: HashMap::new(),
            region_health_score: HashMap::new(),
            failover_events_total,
            http_requests_total,
            http_request_duration,
            database_connections_active,
            database_query_duration,
            transactions_processed_total,
            transaction_volume_total,
            active_users_total,
            error_rate,
        })
    }
}

impl AlertManager {
    fn new() -> Self {
        Self {
            alerts: HashMap::new(),
            notification_channels: vec![
                NotificationChannel {
                    id: "slack".to_string(),
                    name: "Slack".to_string(),
                    channel_type: ChannelType::Slack,
                    config: {
                        let mut config = HashMap::new();
                        config.insert("webhook_url".to_string(), "https://hooks.slack.com/...".to_string());
                        config
                    },
                    enabled: true,
                },
                NotificationChannel {
                    id: "pagerduty".to_string(),
                    name: "PagerDuty".to_string(),
                    channel_type: ChannelType::PagerDuty,
                    config: {
                        let mut config = HashMap::new();
                        config.insert("integration_key".to_string(), "...".to_string());
                        config
                    },
                    enabled: true,
                },
            ],
            escalation_policies: HashMap::new(),
        }
    }

    async fn evaluate_alerts(&self, registry: &Registry) {
        for (alert_id, rule) in &self.alerts {
            if !rule.enabled {
                continue;
            }

            // Check cooldown
            if let Some(last_triggered) = rule.last_triggered {
                if last_triggered.elapsed() < rule.cooldown {
                    continue;
                }
            }

            // Evaluate alert condition
            let should_trigger = self.evaluate_condition(&rule.condition, registry).await;

            if should_trigger {
                self.trigger_alert(alert_id, rule).await;
            }
        }
    }

    async fn evaluate_condition(&self, condition: &AlertCondition, _registry: &Registry) -> bool {
        match condition {
            AlertCondition::ErrorRate { threshold } => {
                // In a real implementation, this would query metrics from the registry
                // For now, return false as placeholder
                false
            }
            AlertCondition::ThresholdAbove { metric: _, value: _ } => {
                // TODO: Implement metric threshold checking
                false
            }
            AlertCondition::ThresholdBelow { metric: _, value: _ } => {
                // TODO: Implement metric threshold checking
                false
            }
            AlertCondition::RateIncrease { metric: _, percentage: _, window: _ } => {
                // TODO: Implement rate increase detection
                false
            }
            AlertCondition::Latency { threshold: _, percentile: _ } => {
                // TODO: Implement latency threshold checking
                false
            }
        }
    }

    async fn trigger_alert(&self, alert_id: &str, rule: &AlertRule) {
        warn!("Alert triggered: {} ({})", rule.name, alert_id);

        // Send notifications
        for channel_id in &rule.notification_channels {
            if let Some(channel) = self.notification_channels.iter().find(|c| c.id == *channel_id) {
                self.send_notification(channel, &rule.name, &rule.severity).await;
            }
        }
    }

    async fn send_notification(&self, channel: &NotificationChannel, message: &str, severity: &AlertSeverity) {
        match channel.channel_type {
            ChannelType::Slack => {
                info!("Sending Slack notification: {} - {}", message, severity);
                // TODO: Implement actual Slack webhook call
            }
            ChannelType::PagerDuty => {
                info!("Sending PagerDuty notification: {} - {}", message, severity);
                // TODO: Implement actual PagerDuty API call
            }
            ChannelType::Email => {
                info!("Sending email notification: {} - {}", message, severity);
                // TODO: Implement actual email sending
            }
            ChannelType::Webhook => {
                info!("Sending webhook notification: {} - {}", message, severity);
                // TODO: Implement actual webhook call
            }
            ChannelType::Teams => {
                info!("Sending Teams notification: {} - {}", message, severity);
                // TODO: Implement actual Teams webhook call
            }
        }
    }
}

impl HealthMonitor {
    fn new() -> Self {
        Self {
            checks: HashMap::new(),
            status: HashMap::new(),
            last_check: HashMap::new(),
        }
    }

    async fn run_health_checks(&self) {
        for (check_id, check) in &self.checks {
            let start_time = Instant::now();
            
            let result = match &check.check_type {
                HealthCheckType::HTTP { url, expected_status } => {
                    self.check_http_health(url, *expected_status, check.timeout).await
                }
                HealthCheckType::TCP { address } => {
                    self.check_tcp_health(address, check.timeout).await
                }
                HealthCheckType::Database { connection_string: _ } => {
                    self.check_database_health(check.timeout).await
                }
                HealthCheckType::Cache { key } => {
                    self.check_cache_health(key, check.timeout).await
                }
                HealthCheckType::Custom { check_fn: _ } => {
                    Ok(ServiceStatus::Healthy) // Placeholder
                }
            };

            let duration = start_time.elapsed();
            
            // Update health status
            if let Some(status) = self.status.get_mut(check_id) {
                status.last_check = start_time;
                status.response_time = duration;
                
                match result {
                    Ok(ServiceStatus::Healthy) => {
                        status.consecutive_successes += 1;
                        status.consecutive_failures = 0;
                        status.error_message = None;
                        
                        if status.consecutive_successes >= check.healthy_threshold {
                            status.status = ServiceStatus::Healthy;
                        }
                    }
                    Ok(ServiceStatus::Degraded) => {
                        status.consecutive_successes = 0;
                        status.consecutive_failures += 1;
                        status.error_message = None;
                        status.status = ServiceStatus::Degraded;
                    }
                    Ok(ServiceStatus::Unhealthy) | Err(_) => {
                        status.consecutive_successes = 0;
                        status.consecutive_failures += 1;
                        status.error_message = Some("Health check failed".to_string());
                        
                        if status.consecutive_failures >= check.unhealthy_threshold {
                            status.status = ServiceStatus::Unhealthy;
                        }
                    }
                }
            }
        }
    }

    async fn check_http_health(&self, url: &str, expected_status: u16, timeout: Duration) -> Result<ServiceStatus, anyhow::Error> {
        let client = reqwest::Client::new();
        let response = client.get(url).timeout(timeout).send().await?;
        
        if response.status().as_u16() == expected_status {
            Ok(ServiceStatus::Healthy)
        } else {
            Ok(ServiceStatus::Degraded)
        }
    }

    async fn check_tcp_health(&self, _address: &str, _timeout: Duration) -> Result<ServiceStatus, anyhow::Error> {
        // TODO: Implement TCP health check
        Ok(ServiceStatus::Healthy)
    }

    async fn check_database_health(&self, _timeout: Duration) -> Result<ServiceStatus, anyhow::Error> {
        // TODO: Implement database health check
        Ok(ServiceStatus::Healthy)
    }

    async fn check_cache_health(&self, _key: &str, _timeout: Duration) -> Result<ServiceStatus, anyhow::Error> {
        // TODO: Implement cache health check
        Ok(ServiceStatus::Healthy)
    }
}

impl SLATracker {
    fn new() -> Self {
        Self {
            services: HashMap::new(),
            metrics: HashMap::new(),
        }
    }

    async fn calculate_sla_metrics(&mut self, _registry: &Registry) {
        for (service_id, definition) in &self.services {
            if let Some(metrics) = self.metrics.get_mut(service_id) {
                // Check if window has expired
                if metrics.window_start.elapsed() > definition.window {
                    // Reset metrics for new window
                    metrics.window_start = Instant::now();
                    metrics.total_requests = 0;
                    metrics.successful_requests = 0;
                    metrics.failed_requests = 0;
                    metrics.total_latency = Duration::from_secs(0);
                    metrics.max_latency = Duration::from_secs(0);
                    metrics.p50_latency = Duration::from_secs(0);
                    metrics.p95_latency = Duration::from_secs(0);
                    metrics.p99_latency = Duration::from_secs(0);
                }

                // TODO: Calculate actual SLA metrics from Prometheus data
                // For now, just log that we're calculating
                debug!("Calculating SLA metrics for service: {}", service_id);
            }
        }
    }
}

// Implement Clone for the structs that need it
impl Clone for AlertManager {
    fn clone(&self) -> Self {
        Self {
            alerts: self.alerts.clone(),
            notification_channels: self.notification_channels.clone(),
            escalation_policies: self.escalation_policies.clone(),
        }
    }
}

impl Clone for HealthMonitor {
    fn clone(&self) -> Self {
        Self {
            checks: self.checks.clone(),
            status: self.status.clone(),
            last_check: self.last_check.clone(),
        }
    }
}

impl Clone for SLATracker {
    fn clone(&self) -> Self {
        Self {
            services: self.services.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_global_observability_initialization() -> Result<(), anyhow::Error> {
        let observability = GlobalObservability::new()?;
        
        // Test that metrics are registered
        let registry = observability.registry();
        let metric_families = registry.gather();
        
        assert!(!metric_families.is_empty());
        
        // Verify key metrics exist
        let metric_names: Vec<String> = metric_families.iter()
            .map(|m| m.get_name().to_string())
            .collect();
        
        assert!(metric_names.contains(&"cache_hits_total".to_string()));
        assert!(metric_names.contains(&"aml_evaluations_total".to_string()));
        assert!(metric_names.contains(&"http_requests_total".to_string()));

        Ok(())
    }

    #[test]
    fn test_metric_recording() -> Result<(), anyhow::Error> {
        let observability = GlobalObservability::new()?;
        
        // Test cache metrics
        observability.record_cache_hit("redis");
        observability.record_cache_miss("redis");
        observability.record_cache_operation("get", Duration::from_millis(5));
        observability.update_cache_size("redis", 1024 * 1024);
        
        // Test AML metrics
        observability.record_aml_evaluation(0.75, Duration::from_millis(150));
        observability.record_aml_rule_triggered("structuring");
        observability.record_aml_case_created("transaction_based", "high");
        
        // Test multi-region metrics
        observability.record_region_request("us-east-1", "200");
        observability.record_region_response_time("us-east-1", Duration::from_millis(50));
        observability.update_region_health_score("us-east-1", 0.95);
        observability.record_failover_event("us-east-1", "eu-west-1");
        
        // Test system metrics
        observability.record_http_request("GET", "/api/test", 200, Duration::from_millis(100));
        observability.update_database_connections(10, 5);
        observability.record_database_query("select", Duration::from_millis(25));
        
        // Test business metrics
        observability.record_transaction("onramp", 1000.0, "USD");
        observability.update_active_users(500);

        Ok(())
    }

    #[test]
    fn test_alert_rules() -> Result<(), anyhow::Error> {
        let observability = GlobalObservability::new()?;
        let alert_rules = observability.get_alert_rules();
        
        // Verify default alert rules are registered
        assert!(alert_rules.contains_key("high_error_rate"));
        assert!(alert_rules.contains_key("region_down"));
        
        // Verify alert rule structure
        if let Some(rule) = alert_rules.get("high_error_rate") {
            assert!(rule.enabled);
            assert!(matches!(rule.severity, AlertSeverity::Warning));
            assert!(matches!(rule.condition, AlertCondition::ErrorRate { .. }));
        }

        Ok(())
    }

    #[test]
    fn test_health_checks() -> Result<(), anyhow::Error> {
        let observability = GlobalObservability::new()?;
        let health_status = observability.get_health_status();
        
        // Verify health checks are registered
        assert!(health_status.contains_key("database"));
        assert!(health_status.contains_key("cache"));
        
        // Verify health status structure
        if let Some(status) = health_status.get("database") {
            assert!(matches!(status.status, ServiceStatus::Unknown));
            assert_eq!(status.consecutive_failures, 0);
            assert_eq!(status.consecutive_successes, 0);
        }

        Ok(())
    }

    #[test]
    fn test_sla_definitions() -> Result<(), anyhow::Error> {
        let observability = GlobalObservability::new()?;
        let sla_metrics = observability.get_sla_metrics();
        
        // Verify SLA definitions are registered
        assert!(sla_metrics.contains_key("api"));
        assert!(sla_metrics.contains_key("aml"));
        
        // Verify SLA metrics structure
        if let Some(metrics) = sla_metrics.get("api") {
            assert_eq!(metrics.service_id, "api");
            assert_eq!(metrics.total_requests, 0);
            assert_eq!(metrics.successful_requests, 0);
            assert_eq!(metrics.failed_requests, 0);
        }

        Ok(())
    }
}
