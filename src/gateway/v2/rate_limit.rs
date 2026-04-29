//! Gateway v2 — Rate-limit service.
//!
//! Token-bucket rate limiter keyed by client IP.  Uses `std::sync::Mutex`
//! over a `HashMap` so it compiles without a Redis dependency — a
//! distributed backend can be swapped in behind the `RateLimiter` trait.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Per-IP bucket state.
struct Bucket {
    tokens: u32,
    last_refill: Instant,
}

/// Shared rate-limiter state.
pub struct RateLimiter {
    capacity: u32,
    refill_interval: Duration,
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl RateLimiter {
    /// Create a new limiter.
    ///
    /// * `capacity` — maximum requests per `refill_interval`.
    /// * `refill_interval` — window after which the bucket is fully refilled.
    pub fn new(capacity: u32, refill_interval: Duration) -> Self {
        Self {
            capacity,
            refill_interval,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` if the request from `ip` is within the allowed rate.
    pub fn check(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        let bucket = buckets.entry(ip.to_owned()).or_insert(Bucket {
            tokens: self.capacity,
            last_refill: now,
        });

        if now.duration_since(bucket.last_refill) >= self.refill_interval {
            bucket.tokens = self.capacity;
            bucket.last_refill = now;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }
}

/// Axum middleware that enforces per-IP rate limiting.
pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Response {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .unwrap_or("unknown")
        .trim()
        .to_owned();

    if !limiter.check(&ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(serde_json::json!({
                "error": "rate_limit_exceeded",
                "message": "Too many requests. Please slow down."
            })),
        )
            .into_response();
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_requests_within_limit() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60));
        assert!(limiter.check("1.2.3.4"));
        assert!(limiter.check("1.2.3.4"));
        assert!(limiter.check("1.2.3.4"));
    }

    #[test]
    fn blocks_after_capacity_exhausted() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        assert!(limiter.check("5.6.7.8"));
        assert!(limiter.check("5.6.7.8"));
        assert!(!limiter.check("5.6.7.8"));
    }

    #[test]
    fn different_ips_have_independent_buckets() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60));
        assert!(limiter.check("10.0.0.1"));
        assert!(!limiter.check("10.0.0.1"));
        assert!(limiter.check("10.0.0.2")); // different IP — still has tokens
    }
}
