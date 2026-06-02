//! Performance Profiling Module
//! 
//! Provides comprehensive performance profiling for the Aframp backend:
//! - Request execution timeline tracking (microsecond precision)
//! - Memory allocation profiling
//! - Async task monitoring (tokio-console integration)
//! - Slow endpoint detection and alerting
//! - P95/P99 latency tracking

pub mod middleware;
pub mod models;
pub mod service;
pub mod admin;
pub mod metrics;

pub use middleware::ProfilingMiddleware;
pub use models::{
    PerformanceProfile, MemorySnapshot, TraceTally, ProfilingConfig,
    SlowEndpointAlert, EndpointMetrics,
};
pub use service::ProfilingService;
pub use admin::ProfilingAdminService;
pub use metrics::ProfilingMetricsService;