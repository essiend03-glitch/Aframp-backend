//! Performance Profiling Data Models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Performance profile - aggregate endpoint statistics
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PerformanceProfile {
    pub id: Uuid,
    pub endpoint_path: String,
    pub method: String,
    pub p50_duration_ms: Option<f32>,
    pub p95_duration_ms: Option<f32>,
    pub p99_duration_ms: Option<f32>,
    pub avg_duration_ms: Option<f32>,
    pub pub max_duration_ms: Option<f32>,
    pub min_duration_ms: Option<f32>,
    pub request_count: i64,
    pub error_count: i64,
    pub slow_request_count: i64,
    pub memory_allocated_bytes: Option<i64>,
    pub memory_peak_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Memory allocation snapshot
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MemorySnapshot {
    pub id: Uuid,
    pub endpoint_path: String,
    pub allocation_count: i64,
    pub total_bytes_allocated: i64,
    pub total_bytes_deallocated: i64,
    pub peak_bytes_allocated: i64,
    pub avg_allocation_bytes: Option<f32>,
    pub vector_reallocs: i64,
    pub snapshot_time: DateTime<Utc>,
}

/// Trace execution tally
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TraceTally {
    pub id: Uuid,
    pub trace_id: String,
    pub span_name: String,
    pub parent_span_id: Option<String>,
    pub endpoint_path: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: Option<f32>,
    pub poll_count: Option<i32>,
    pub poll_duration_ms: Option<f32>,
    pub blocked_duration_ms: Option<f32>,
    pub scheduled_count: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Profiling configuration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProfilingConfig {
    pub id: Uuid,
    pub sample_rate: f32,
    pub slow_request_threshold_ms: f32,
    pub enable_memory_profiling: bool,
    pub enable_trace_collection: bool,
    pub max_traces_per_minute: i32,
    pub p95_threshold_ms: f32,
    pub p99_threshold_ms: f32,
    pub is_active: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

/// Slow endpoint alert
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SlowEndpointAlert {
    pub id: Uuid,
    pub endpoint_path: String,
    pub method: String,
    pub latency_p95_ms: Option<f32>,
    pub latency_p99_ms: Option<f32>,
    pub alert_threshold_ms: f32,
    pub alert_type: String,
    pub alert_severity: String,
    pub triggered_at: DateTime<Utc>,
    pub acknowledged: bool,
    pub acknowledged_by: Option<Uuid>,
    pub acknowledged_at: Option<DateTime<Utc>>,
}

/// Real-time endpoint metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointMetrics {
    pub endpoint_path: String,
    pub method: String,
    pub request_count: i64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub max_duration_ms: f64,
    pub error_count: i64,
    pub error_rate: f64,
    pub slow_request_count: i64,
    pub memory_peak_bytes: i64,
}

/// Request timing data
#[derive(Debug, Clone)]
pub struct RequestTiming {
    pub trace_id: String,
    pub endpoint_path: String,
    pub method: String,
    pub start_time: std::time::Instant,
    pub start_timestamp: DateTime<Utc>,
    pub end_time: Option<std::time::Instant>,
    pub poll_count: u64,
    pub poll_total_duration: std::time::Duration,
    pub scheduled_count: u64,
    pub memory_before: Option<u64>,
    pub memory_after: Option<u64>,
}

impl RequestTiming {
    pub fn new(endpoint_path: String, method: String) -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            endpoint_path,
            method,
            start_time: std::time::Instant::now(),
            start_timestamp: Utc::now(),
            end_time: None,
            poll_count: 0,
            poll_total_duration: std::time::Duration::ZERO,
            scheduled_count: 0,
            memory_before: None,
            memory_after: None,
        }
    }

    pub fn finish(&mut self) {
        self.end_time = Some(std::time::Instant::now());
    }

    pub fn duration_ms(&self) -> f64 {
        let end = self.end_time.unwrap_or_else(|| std::time::Instant::now());
        (end - self.start_time).as_secs_f64() * 1000.0
    }
}

/// Profiling control request
#[derive(Debug, Deserialize)]
pub struct UpdateProfilingRequest {
    pub sample_rate: Option<f32>,
    pub slow_request_threshold_ms: Option<f32>,
    pub p95_threshold_ms: Option<f32>,
    pub p99_threshold_ms: Option<f32>,
    pub enable_memory_profiling: Option<bool>,
    pub enable_trace_collection: Option<bool>,
    pub max_traces_per_minute: Option<i32>,
    pub is_active: Option<bool>,
    pub updated_by: Uuid,
}

/// API Response types
#[derive(Debug, Serialize)]
pub struct SlowEndpointsResponse {
    pub endpoints: Vec<EndpointMetrics>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProfilingStatusResponse {
    pub is_active: bool,
    pub sample_rate: f32,
    pub traces_per_minute: i32,
    pub memory_profiling_enabled: bool,
    pub trace_collection_enabled: bool,
    pub p95_threshold_ms: f32,
    pub p99_threshold_ms: f32,
}