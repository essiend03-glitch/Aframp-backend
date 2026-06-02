//! Performance Profiling Service
//! Core profiling logic and data management

use crate::profiling::models::{
    EndpointMetrics, PerformanceProfile, ProfilingConfig, RequestTiming,
    SlowEndpointAlert,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Thread pool for CPU-bound crypto operations
pub struct CryptoThreadPool {
    workers: Arc<tokio::task::JoinSet<()>>,
}

impl CryptoThreadPool {
    pub fn new(threads: usize) -> Self {
        let workers = Arc::new(tokio::task::JoinSet::new());
        
        // Pre-spawn workers
        for _ in 0..threads {
            workers.spawn(async {
                // Worker idle loop - would process crypto tasks from queue
                tokio::time::sleep(std::time::Duration::MAX).await;
            });
        }

        Self { workers }
    }

    /// Execute CPU-bound operation on thread pool
    pub async fn spawn_blocking<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        tokio::task::spawn_blocking(f).await.unwrap()
    }
}

/// Profiling service configuration
#[derive(Debug, Clone)]
pub struct ProfilingServiceConfig {
    pub sample_rate: f32,
    pub slow_request_threshold_ms: f32,
    pub p95_threshold_ms: f32,
    pub p99_threshold_ms: f32,
    pub enable_memory_profiling: bool,
    pub enable_trace_collection: bool,
    pub max_traces_per_minute: i32,
    /// Pre-allocated capacity for timing vectors
    pub timing_vector_capacity: usize,
    /// Pre-allocated capacity for endpoint metrics
    pub metrics_vector_capacity: usize,
}

impl Default for ProfilingServiceConfig {
    fn default() -> Self {
        Self {
            sample_rate: 0.01,           // 1% sample rate
            slow_request_threshold_ms: 100.0,
            p95_threshold_ms: 25.0,
            p99_threshold_ms: 100.0,
            enable_memory_profiling: false,
            enable_trace_collection: true,
            max_traces_per_minute: 1000,
            timing_vector_capacity: 10000,
            metrics_vector_capacity: 1000,
        }
    }
}

/// Profiling Service - manages request timing collection and analysis
pub struct ProfilingService {
    config: ProfilingServiceConfig,
    // Pre-allocated vectors to minimize heap reallocations
    recent_timings: Arc<RwLock<VecDeque<RequestTiming>>>,
    endpoint_metrics: Arc<RwLock<std::collections::HashMap<String, EndpointMetrics>>>,
    crypto_pool: CryptoThreadPool,
}

impl ProfilingService {
    pub fn new(config: ProfilingServiceConfig) -> Self {
        // Pre-allocate with capacity to minimize reallocations
        let timings = VecDeque::with_capacity(config.timing_vector_capacity);
        let metrics = std::collections::HashMap::with_capacity(config.metrics_vector_capacity);

        Self {
            config,
            recent_timings: Arc::new(RwLock::new(timings)),
            endpoint_metrics: Arc::new(RwLock::new(metrics)),
            crypto_pool: CryptoThreadPool::new(4), // 4 crypto workers
        }
    }

    /// Record a request timing - non-blocking
    pub async fn record_request(
        &self,
        endpoint: &str,
        method: &str,
        duration_ms: f64,
        timing: RequestTiming,
    ) -> Result<(), ProfilingError> {
        // Store recent timing
        {
            let mut timings = self.recent_timings.write().await;
            if timings.len() >= self.config.timing_vector_capacity {
                // Remove oldest when at capacity (FIFO)
                timings.pop_front();
            }
            timings.push_back(timing);
        }

        // Update endpoint metrics
        let key = format!("{}:{}", method, endpoint);
        {
            let mut metrics = self.endpoint_metrics.write().await;
            let entry = metrics.entry(key.clone()).or_insert_with(|| {
                EndpointMetrics {
                    endpoint_path: endpoint.to_string(),
                    method: method.to_string(),
                    request_count: 0,
                    avg_duration_ms: 0.0,
                    p50_duration_ms: 0.0,
                    p95_duration_ms: 0.0,
                    p99_duration_ms: 0.0,
                    max_duration_ms: 0.0,
                    error_count: 0,
                    error_rate: 0.0,
                    slow_request_count: 0,
                    memory_peak_bytes: 0,
                }
            });

            // Update with streaming calculation
            entry.request_count += 1;
            entry.avg_duration_ms = ((entry.avg_duration_ms * (entry.request_count - 1) as f64) 
                + duration_ms) / entry.request_count as f64;
            entry.max_duration_ms = entry.max_duration_ms.max(duration_ms);

            // Check if slow request
            if duration_ms > self.config.slow_request_threshold_ms as f64 {
                entry.slow_request_count += 1;
            }
        }

        // Trigger alert if thresholds exceeded
        self.check_and_alert(endpoint, method, duration_ms).await;

        Ok(())
    }

    /// Calculate percentiles from recent timings for an endpoint
    pub async fn calculate_percentiles(
        &self,
        endpoint: &str,
    ) -> Option<(f64, f64, f64)> {
        let timings = self.recent_timings.read().await;
        
        // Filter timings for this endpoint
        let endpoint_timings: Vec<f64> = timings
            .iter()
            .filter(|t| t.endpoint_path == endpoint)
            .map(|t| t.duration_ms())
            .collect();

        if endpoint_timings.is_empty() {
            return None;
        }

        // Sort for percentile calculation
        let mut sorted = endpoint_timings;
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let n = sorted.len();
        let p95_idx = ((n as f64) * 0.95).ceil() as usize - 1;
        let p99_idx = ((n as f64) * 0.99).ceil() as usize - 1;

        Some((
            sorted[n / 2],        // P50
            sorted[p95_idx.min(n-1)], // P95
            sorted[p99_idx.min(n-1)], // P99
        ))
    }

    /// Check if alert should be triggered
    async fn check_and_alert(
        &self,
        endpoint: &str,
        method: &str,
        duration_ms: f64,
    ) {
        let mut should_alert = false;
        let mut alert_type = String::new();
        let mut alert_severity = String::new();

        if duration_ms > self.config.p99_threshold_ms as f64 {
            should_alert = true;
            alert_type = "p99_exceeded".to_string();
            alert_severity = "critical".to_string();
        } else if duration_ms > self.config.p95_threshold_ms as f64 {
            should_alert = true;
            alert_type = "p95_exceeded".to_string();
            alert_severity = "warning".to_string();
        }

        if should_alert {
            warn!(
                endpoint = %endpoint,
                method = %method,
                duration_ms = duration_ms,
                alert_type = %alert_type,
                "Slow endpoint detected"
            );
            // In production, would persist alert to database
        }
    }

    /// Get slow endpoints exceeding thresholds
    pub async fn get_slow_endpoints(&self) -> Vec<EndpointMetrics> {
        let metrics = self.endpoint_metrics.read().await;
        
        let mut slow_endpoints: Vec<EndpointMetrics> = metrics
            .values()
            .filter(|m| {
                m.p95_duration_ms > self.config.p95_threshold_ms as f64
                    || m.p99_duration_ms > self.config.p99_threshold_ms as f64
            })
            .cloned()
            .collect();

        // Sort by P99 latency descending
        slow_endpoints.sort_by(|a, b| {
            b.p99_duration_ms.partial_cmp(&a.p99_duration_ms).unwrap_or(
                std::cmp::Ordering::Equal
            )
        });

        slow_endpoints
    }

    /// Update profiling configuration dynamically
    pub async fn update_config(&mut self, config: ProfilingServiceConfig) {
        info!(
            sample_rate = config.sample_rate,
            p95_threshold_ms = config.p95_threshold_ms,
            p99_threshold_ms = config.p99_threshold_ms,
            "Profiling configuration updated"
        );
        self.config = config;
    }

    /// Execute crypto operation on thread pool (non-blocking)
    pub async fn verify_signature_blocking(
        &self,
        data: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<bool, ProfilingError> {
        // Offload CPU-bound crypto to thread pool
        let result = self.crypto_pool.spawn_blocking(move || {
            // This would be actual signature verification
            // Using mock for example
            if data.is_empty() || signature.is_empty() {
                return false;
            }
            // Simulate verification
            true
        }).await;

        Ok(result)
    }

    /// Get current configuration
    pub fn config(&self) -> &ProfilingServiceConfig {
        &self.config
    }
}

/// Pre-allocated vector for database array mapping
/// Minimizes runtime heap reallocations
pub struct PreallocatedVector<T> {
    data: Vec<T>,
    capacity: usize,
}

impl<T> PreallocatedVector<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: T) {
        if self.data.len() < self.capacity {
            self.data.push(value);
        } else {
            // Replace oldest if at capacity (ring buffer behavior)
            self.data.remove(0);
            self.data.push(value);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<T: Clone> Clone for PreallocatedVector<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            capacity: self.capacity,
        }
    }
}

/// Profiling errors
#[derive(Debug, thiserror::Error)]
pub enum ProfilingError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Alert error: {0}")]
    AlertError(String),
}

impl std::fmt::Display for ProfilingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfilingError::ConfigError(s) => write!(f, "Config: {}", s),
            ProfilingError::DatabaseError(s) => write!(f, "Database: {}", s),
            ProfilingError::AlertError(s) => write!(f, "Alert: {}", s),
        }
    }
}