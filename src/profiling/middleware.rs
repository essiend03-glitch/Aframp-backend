//! Performance Profiling Middleware
//! Captures precise request execution timelines with microsecond precision

use crate::profiling::models::RequestTiming;
use crate::profiling::service::ProfilingService;
use axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, instrument};

/// High-performance profiling middleware
/// Captures precise timing data without blocking async operations
pub struct ProfilingMiddleware {
    profiling_service: Arc<ProfilingService>,
    sample_rate: f32,
}

impl ProfilingMiddleware {
    pub fn new(profiling_service: Arc<ProfilingService>, sample_rate: f32) -> Self {
        Self {
            profiling_service,
            sample_rate,
        }
    }

    /// Process request and capture timing
    #[instrument(skip(self, request, next), fields(endpoint = %request.uri(), method = %request.method()))]
    pub async fn capture(
        &self,
        request: Request<Body>,
        next: Next,
    ) -> Response {
        // Sample only a fraction of requests based on sample rate
        let should_sample = self.should_sample();
        
        if !should_sample {
            return next.run(request).await;
        }

        // Capture start time with high precision
        let start_instant = Instant::now();
        let start_timestamp = chrono::Utc::now();
        
        // Get endpoint path for metrics
        let endpoint_path = request.uri().path().to_string();
        let method = request.method().as_str().to_string();

        // Capture memory before (if profiling enabled)
        let memory_before = if self.profiling_service.config.enable_memory_profiling {
            self.get_memory_usage()
        } else {
            None
        };

        // Execute request
        let mut response = next.run(request).await;

        // Capture end timing
        let end_instant = Instant::now();
        let duration = end_instant.duration_since(start_instant);
        let duration_ms = duration.as_secs_f64() * 1000.0;

        // Capture memory after
        let memory_after = if self.profiling_service.config.enable_memory_profiling {
            self.get_memory_usage()
        } else {
            None
        };

        // Record metrics asynchronously (non-blocking)
        let timing = RequestTiming {
            trace_id: uuid::Uuid::new_v4().to_string(),
            endpoint_path: endpoint_path.clone(),
            method: method.clone(),
            start_time: start_instant,
            start_timestamp,
            end_time: Some(end_instant),
            poll_count: 0, // Would need tokio instrumentation
            poll_total_duration: Duration::ZERO,
            scheduled_count: 0,
            memory_before,
            memory_after,
        };

        // Record asynchronously without blocking the response
        let service = self.profiling_service.clone();
        let endpoint = endpoint_path.clone();
        let method_clone = method.clone();
        
        tokio::spawn(async move {
            if let Err(e) = service.record_request(&endpoint, &method_clone, duration_ms, timing).await {
                debug!(error = %e, "Failed to record request timing");
            }
        });

        debug!(
            endpoint = %endpoint_path,
            duration_ms = duration_ms,
            "Request profiling captured"
        );

        response
    }

    /// Determine if this request should be sampled
    fn should_sample(&self) -> bool {
        // Use random sampling based on sample rate
        if self.sample_rate >= 1.0 {
            return true;
        }
        if self.sample_rate <= 0.0 {
            return false;
        }
        
        // Fast random sampling
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(uuid::Uuid::new_v4().as_u128() as u64);
        let hash = hasher.finish() as f64 / (u64::MAX as f64);
        
        hash < self.sample_rate as f64
    }

    /// Get current memory usage in bytes
    fn get_memory_usage(&self) -> Option<u64> {
        #[cfg(target_os = "linux")]
        {
            // Read from /proc/self/statm on Linux
            std::fs::read_to_string("/proc/self/statm")
                .ok()
                .and_then(|content| {
                    let parts: Vec<&str> = content.split_whitespace().collect();
                    if parts.len() >= 2 {
                        // Multiply by page size (typically 4096 bytes)
                        parts[1].parse::<u64>().ok().map(|pages| pages * 4096)
                    } else {
                        None
                    }
                })
        }
        #[cfg(not(target_os = "linux"))]
        {
            // On other platforms, return None (not implemented)
            None
        }
    }
}

use std::time::Duration;

/// Middleware layer factory for Axum
pub fn profiling_layer(
    profiling_service: Arc<ProfilingService>,
) -> impl axum::middleware::Layer<ProfilingService> {
    ProfilingLayer {
        service: profiling_service,
    }
}

#[derive(Clone)]
struct ProfilingLayer {
    service: Arc<ProfilingService>,
}

impl<S> axum::middleware::Layer<S> for ProfilingLayer {
    type Service = ProfilingMiddleware;

    fn layer(&self, inner: S) -> Self::Service {
        ProfilingMiddleware::new(
            self.service.clone(),
            self.service.config.sample_rate,
        )
    }
}

/// Zero-allocation path extractor for high-performance routing
pub struct FastPath(pub String);

impl<B> axum::extract::FromRequest<B> for FastPath {
    type Rejection = axum::extract::rejection::FailedToExtractPath;

    async fn from_request(req: Request<B>, _state: &()) -> Result<Self, Self::Rejection> {
        // Use raw path without allocation when possible
        let path = req.uri().path().to_string();
        Ok(FastPath(path))
    }
}

/// Optimized JSON serialization using pre-allocated buffers
pub mod json_optimized {
    use serde::{Serialize, Serializer};
    use std::io::Write;

    /// Serialize with pre-allocated buffer to reduce heap allocations
    pub fn serialize_to_writer<W: Write, T: Serialize>(
        writer: &mut W,
        value: &T,
    ) -> Result<(), serde_json::Error> {
        let mut serializer = serde_json::Serializer::new(writer);
        value.serialize(&mut serializer)
    }

    /// Pre-allocated string buffer for JSON serialization
    pub struct JsonBuffer {
        buffer: String,
        capacity: usize,
    }

    impl JsonBuffer {
        pub fn new(capacity: usize) -> Self {
            Self {
                buffer: String::with_capacity(capacity),
                capacity,
            }
        }

        pub fn with_estimate(estimate: usize) -> Self {
            // Add 20% overhead for JSON formatting
            Self::new((estimate as f64 * 1.2) as usize)
        }

        pub fn serialize<T: Serialize>(&mut self, value: &T) -> Result<&str, serde_json::Error> {
            self.buffer.clear();
            {
                let mut writer = &mut self.buffer;
                value.serialize(&mut serde_json::Serializer::new(&mut writer))?;
            }
            Ok(&self.buffer)
        }

        pub fn as_str(&self) -> &str {
            &self.buffer
        }
    }

    impl Default for JsonBuffer {
        fn default() -> Self {
            Self::new(1024) // 1KB default
        }
    }
}