//! Axum middleware that applies HTB rate-limiting and returns HTTP 429 with Retry-After.

use std::sync::Arc;
use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use uuid::Uuid;

use super::{models::RateLimitDecision, token_bucket::TokenBucketLimiter, models::TenantSlaProfile};
use crate::rate_engine::metrics::THROTTLES_TOTAL;

/// Shared state injected via `Extension`.
pub struct RateLimitState {
    pub limiter: Arc<TokenBucketLimiter>,
}

/// Resolve tenant ID from `X-Tenant-Id` header; fall back to a default.
fn extract_tenant_id(req: &Request<Body>) -> Uuid {
    req.headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::nil)
}

pub async fn rate_limit_layer(
    Extension(state): Extension<Arc<RateLimitState>>,
    Extension(profile): Extension<Arc<TenantSlaProfile>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    match state.limiter.check(&profile).await {
        RateLimitDecision::Allow => next.run(req).await,
        RateLimitDecision::Throttle { retry_after_ms } => {
            THROTTLES_TOTAL
                .with_label_values(&[&profile.tenant_id.to_string(), &profile.tier])
                .inc();
            let retry_secs = (retry_after_ms as f64 / 1000.0).ceil() as u64;
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, retry_secs.to_string())],
                format!("{{\"error\":\"rate_limit_exceeded\",\"retry_after_ms\":{retry_after_ms}}}"),
            )
                .into_response()
        }
    }
}
