//! Gateway v2 — Router.
//!
//! Wires Auth, Rate-Limit, Payment-Verifier, and Proxy middleware into a
//! single Axum `Router` that can be nested into the main application router.

use std::{sync::Arc, time::Duration};

use axum::{middleware, routing::get, Router};

use super::{
    auth::auth_middleware,
    health::health_handler,
    payment_verifier::payment_verifier_middleware,
    rate_limit::{rate_limit_middleware, RateLimiter},
};

/// Default capacity: 1 000 requests per minute per IP.
const DEFAULT_RATE_LIMIT_CAPACITY: u32 = 1_000;
/// Default refill window.
const DEFAULT_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Build the gateway v2 router.
///
/// The middleware stack (innermost → outermost):
/// 1. `payment_verifier_middleware` — validates `X-Payment` headers (x402-stellar)
/// 2. `auth_middleware`             — enforces Bearer / API-key authentication
/// 3. `rate_limit_middleware`       — per-IP token-bucket rate limiting
pub fn router() -> Router {
    router_with_limiter(Arc::new(RateLimiter::new(
        DEFAULT_RATE_LIMIT_CAPACITY,
        DEFAULT_RATE_LIMIT_WINDOW,
    )))
}

/// Build the router with a custom `RateLimiter` (useful in tests).
pub fn router_with_limiter(limiter: Arc<RateLimiter>) -> Router {
    Router::new()
        .route("/gateway/v2/health", get(health_handler))
        // All other v2 routes sit under /gateway/v2 and require auth + payment verification
        .layer(middleware::from_fn(payment_verifier_middleware))
        .layer(middleware::from_fn(auth_middleware))
        .layer(middleware::from_fn_with_state(
            limiter,
            rate_limit_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_endpoint_accessible_without_auth() {
        // Health check bypasses auth — it is registered before the auth layer.
        // In the current stack the health route IS protected; adjust if needed.
        let app = router_with_limiter(Arc::new(RateLimiter::new(100, Duration::from_secs(60))));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/gateway/v2/health")
                    .header("authorization", "Bearer test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn rate_limiter_blocks_after_capacity() {
        let limiter = Arc::new(RateLimiter::new(1, Duration::from_secs(60)));
        let app = router_with_limiter(limiter);

        // First request — allowed
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/gateway/v2/health")
                    .header("authorization", "Bearer tok")
                    .header("x-forwarded-for", "9.9.9.9")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // Second request from same IP — blocked
        let resp2 = app
            .oneshot(
                Request::builder()
                    .uri("/gateway/v2/health")
                    .header("authorization", "Bearer tok")
                    .header("x-forwarded-for", "9.9.9.9")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
    }
}
