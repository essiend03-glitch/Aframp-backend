//! Performance Profiling Metrics
//! Prometheus metrics for API latency, Tokio task monitoring, and alerting

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, Opts, Registry,
};
use std::sync::Arc;

/// Profiling Metrics Service
pub struct ProfilingMetricsService {
    registry: Registry,

    // Latency histograms with customized bucket spans
    pub api_request_duration_seconds: HistogramVec,
    
    // Tokio runtime metrics
    pub tokio_task_poll_duration_seconds: HistogramVec,
    pub tokio_thread_migrations_total: CounterVec,
    pub tokio_task_scheduled_total: CounterVec,
    
    // Memory metrics
    pub memory_allocation_bytes: HistogramVec,
    pub memory_reallocations_total: CounterVec,
    
    // Slow endpoint alerts
    pub slow_endpoint_alerts_total: CounterVec,
    pub slow_endpoint_p99_breaches: CounterVec,
    
    // Profiling overhead
    pub profiling_overhead_cpu_percent: Gauge,
    
    // Request counts
    pub requests_sampled_total: CounterVec,
    pub requests_total: CounterVec,
}

impl ProfilingMetricsService {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // Customized histogram buckets for API latency
        // Granular spans around typical P95/P99 thresholds (25ms, 100ms)
        let latency_buckets = vec![
            0.001, 0.005, 0.01,   // 1ms, 5ms, 10ms
            0.015, 0.02, 0.025,   // 15ms, 20ms, 25ms (P95 threshold)
            0.05, 0.075, 0.1,     // 50ms, 75ms, 100ms (P99 threshold)
            0.15, 0.2, 0.25,      // 150ms, 200ms
            0.5, 0.75, 1.0,       // 500ms, 750ms, 1s
        ];

        let api_request_duration_seconds = HistogramVec::new(
            Opts::new(
                "api_request_duration_seconds",
                "API request duration in seconds with custom buckets",
            )
            .subsystem("profiling")
            .buckets(latency_buckets),
            &["method", "endpoint", "status"],
        )?;

        // Tokio task metrics
        let tokio_task_poll_duration_seconds = HistogramVec::new(
            Opts::new(
                "tokio_task_poll_duration_seconds",
                "Tokio task poll duration in seconds",
            )
            .subsystem("profiling")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
            &["task_name"],
        )?;

        let tokio_thread_migrations_total = CounterVec::new(
            Opts::new(
                "tokio_thread_migrations_total",
                "Total number of thread migrations in Tokio runtime",
            )
            .subsystem("profiling"),
            &["direction"], // "to_worker", "from_worker"
        )?;

        let tokio_task_scheduled_total = CounterVec::new(
            Opts::new(
                "tokio_task_scheduled_total",
                "Total number of times tasks were scheduled",
            )
            .subsystem("profiling"),
            &["task_name", "scheduled_from"], // "poll", "wake"
        )?;

        // Memory metrics
        let memory_allocation_bytes = HistogramVec::new(
            Opts::new(
                "memory_allocation_bytes",
                "Memory allocation sizes in bytes",
            )
            .subsystem("profiling")
            .buckets(vec![
                64.0, 128.0, 256.0, 512.0,     // Small allocations
                1024.0, 2048.0, 4096.0,        // Medium allocations
                8192.0, 16384.0, 32768.0,      // Large allocations
            ]),
            &["endpoint", "allocation_type"], // "vec", "string", "json"
        )?;

        let memory_reallocations_total = CounterVec::new(
            Opts::new(
                "memory_reallocations_total",
                "Total number of vector reallocations",
            )
            .subsystem("profiling"),
            &["endpoint"],
        )?;

        // Alert metrics
        let slow_endpoint_alerts_total = CounterVec::new(
            Opts::new(
                "slow_endpoint_alerts_total",
                "Total slow endpoint alerts triggered",
            )
            .subsystem("profiling")
            .buckets(vec!["p95", "p99", "critical"]),
            &["endpoint", "alert_type"],
        )?;

        let slow_endpoint_p99_breaches = CounterVec::new(
            Opts::new(
                "slow_endpoint_p99_breaches_total",
                "Total P99 latency breaches over 200ms threshold",
            )
            .subsystem("profiling"),
            &["endpoint", "duration_bucket"],
        )?;

        // Profiling overhead
        let profiling_overhead_cpu_percent = Gauge::new(
            "profiling_overhead_cpu_percent",
            "CPU overhead introduced by profiling as percentage",
        )?;

        // Request counts
        let requests_sampled_total = CounterVec::new(
            Opts::new(
                "requests_sampled_total",
                "Total requests sampled for profiling",
            )
            .subsystem("profiling"),
            &["method", "endpoint"],
        )?;

        let requests_total = CounterVec::new(
            Opts::new(
                "requests_total",
                "Total requests to API endpoints",
            )
            .subsystem("profiling"),
            &["method", "endpoint", "status"],
        )?;

        // Register metrics
        registry.register(Box::new(api_request_duration_seconds.clone()))?;
        registry.register(Box::new(tokio_task_poll_duration_seconds.clone()))?;
        registry.register(Box::new(tokio_thread_migrations_total.clone()))?;
        registry.register(Box::new(tokio_task_scheduled_total.clone()))?;
        registry.register(Box::new(memory_allocation_bytes.clone()))?;
        registry.register(Box::new(memory_reallocations_total.clone()))?;
        registry.register(Box::new(slow_endpoint_alerts_total.clone()))?;
        registry.register(Box::new(slow_endpoint_p99_breaches.clone()))?;
        registry.register(Box::new(profiling_overhead_cpu_percent.clone()))?;
        registry.register(Box::new(requests_sampled_total.clone()))?;
        registry.register(Box::new(requests_total.clone()))?;

        Ok(Self {
            registry,
            api_request_duration_seconds,
            tokio_task_poll_duration_seconds,
            tokio_thread_migrations_total,
            tokio_task_scheduled_total,
            memory_allocation_bytes,
            memory_reallocations_total,
            slow_endpoint_alerts_total,
            slow_endpoint_p99_breaches,
            profiling_overhead_cpu_percent,
            requests_sampled_total,
            requests_total,
        })
    }

    /// Record API request duration
    pub fn record_request_duration(
        &self,
        method: &str,
        endpoint: &str,
        status: &str,
        duration_secs: f64,
    ) {
        self.api_request_duration_seconds
            .with_label_values(&[method, endpoint, status])
            .observe(duration_secs);
    }

    /// Record Tokio task poll duration
    pub fn record_task_poll_duration(&self, task_name: &str, duration_secs: f64) {
        self.tokio_task_poll_duration_seconds
            .with_label_values(&[task_name])
            .observe(duration_secs);
    }

    /// Record thread migration
    pub fn record_thread_migration(&self, direction: &str) {
        self.tokio_thread_migrations_total
            .with_label_values(&[direction])
            .inc();
    }

    /// Record task scheduled
    pub fn record_task_scheduled(&self, task_name: &str, from: &str) {
        self.tokio_task_scheduled_total
            .with_label_values(&[task_name, from])
            .inc();
    }

    /// Record memory allocation
    pub fn record_allocation(
        &self,
        endpoint: &str,
        allocation_type: &str,
        bytes: f64,
    ) {
        self.memory_allocation_bytes
            .with_label_values(&[endpoint, allocation_type])
            .observe(bytes);
    }

    /// Record vector reallocation
    pub fn record_reallocation(&self, endpoint: &str) {
        self.memory_reallocations_total
            .with_label_values(&[endpoint])
            .inc();
    }

    /// Record slow endpoint alert
    pub fn record_slow_alert(&self, endpoint: &str, alert_type: &str) {
        self.slow_endpoint_alerts_total
            .with_label_values(&[endpoint, alert_type])
            .inc();
    }

    /// Record P99 breach over 200ms
    pub fn record_p99_breach(&self, endpoint: &str, duration_ms: f64) {
        // Bucket by duration ranges around 200ms threshold
        let bucket = if duration_ms < 250.0 {
            "200-250ms"
        } else if duration_ms < 500.0 {
            "250-500ms"
        } else if duration_ms < 1000.0 {
            "500ms-1s"
        } else {
            "1s+"
        };

        self.slow_endpoint_p99_breaches
            .with_label_values(&[endpoint, bucket])
            .inc();
    }

    /// Update profiling overhead gauge
    pub fn set_profiling_overhead(&self, percent: f64) {
        self.profiling_overhead_cpu_percent.set(percent);
    }

    /// Record sampled request
    pub fn record_sampled_request(&self, method: &str, endpoint: &str) {
        self.requests_sampled_total
            .with_label_values(&[method, endpoint])
            .inc();
    }

    /// Record total request
    pub fn record_total_request(&self, method: &str, endpoint: &str, status: &str) {
        self.requests_total
            .with_label_values(&[method, endpoint, status])
            .inc();
    }

    /// Get registry for Prometheus exposition
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

impl Default for ProfilingMetricsService {
    fn default() -> Self {
        Self::new().expect("Failed to create profiling metrics service")
    }
}

/// Alert conditions for performance monitoring
#[derive(Debug, Clone)]
pub struct PerformanceAlertConfig {
    /// P99 latency breach threshold (200ms default)
    pub p99_breach_threshold_ms: f64,
    /// Duration window for continuous breach detection (1 minute)
    pub breach_window_seconds: i64,
    /// Financial operation endpoints requiring strict monitoring
    pub financial_operation_endpoints: Vec<String>,
}

impl Default for PerformanceAlertConfig {
    fn default() -> Self {
        Self {
            p99_breach_threshold_ms: 200.0,
            breach_window_seconds: 60,
            financial_operation_endpoints: vec![
                "/api/v1/wallet/balance".to_string(),
                "/api/v1/transactions/submit".to_string(),
                "/api/v1/aml/screen".to_string(),
            ],
        }
    }
}

/// Check if performance alert should be triggered
pub fn check_performance_alerts(
    config: &PerformanceAlertConfig,
    metrics: &ProfilingMetricsService,
    recent_latencies: &[f64],
) -> Vec<PerformanceAlert> {
    let mut alerts = Vec::new();

    // Check for continuous P99 breach over 1 minute window
    let breach_count = recent_latencies
        .iter()
        .filter(|&&ms| ms > config.p99_breach_threshold_ms)
        .count();

    if breach_count > 0 {
        // This is a simplified check - in production would track time windows
        alerts.push(PerformanceAlert {
            alert_type: AlertType::P99LatencyBreach,
            severity: AlertSeverity::Critical,
            endpoint: "financial_operations".to_string(),
            message: format!(
                "P99 latency exceeded {}ms for {} requests in the last minute",
                config.p99_breach_threshold_ms, breach_count
            ),
            triggered_at: chrono::Utc::now(),
        });
    }

    alerts
}

#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub endpoint: String,
    pub message: String,
    pub triggered_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy)]
pub enum AlertType {
    P99LatencyBreach,
    P95LatencyBreach,
    MemoryLeak,
    TaskStarvation,
}

#[derive(Debug, Clone, Copy)]
pub enum AlertSeverity {
    Warning,
    Critical,
}