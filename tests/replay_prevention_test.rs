//! Integration tests for Replay Attack Prevention middleware (Issue #141).
//!
//! Requires a running Redis instance.
//! Run with: cargo test --features cache replay_prevention -- --ignored

use anyhow::{Context, Result};
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    response::IntoResponse,
    routing::post,
    Router,
};
use tower::ServiceExt;

use Bitmesh_backend::cache::{init_cache_pool, CacheConfig};
use Bitmesh_backend::middleware::replay_prevention::{
    replay_prevention_middleware, ReplayConfig, ReplayPreventionState,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn build_router(config: ReplayConfig) -> Result<Router> {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let pool = init_cache_pool(CacheConfig {
        redis_url,
        ..Default::default()
    })
    .await
    .context("Redis init failed")?;

    let state = ReplayPreventionState {
        redis: Arc::new(pool),
        config: Arc::new(config),
    };

    Ok(Router::new()
        .route("/transfer", post(|| async { "ok".into_response() }))
        .layer(middleware::from_fn_with_state(
            state,
            replay_prevention_middleware,
        )))
}

fn signed_request(nonce: &str, timestamp: i64, consumer: &str) -> Result<Request<Body>> {
    Ok(Request::builder()
        .method("POST")
        .uri("/transfer")
        .header("x-aframp-timestamp", timestamp.to_string())
        .header("x-aframp-nonce", nonce)
        .header("x-aframp-consumer", consumer)
        .body(Body::empty())?)
}

fn unique_nonce() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// A fresh request with a valid timestamp and unique nonce must be accepted.
#[tokio::test]
#[ignore]
async fn test_first_request_accepted() -> Result<()> {
    let router = build_router(ReplayConfig::default()).await?;
    let now = chrono::Utc::now().timestamp();
    let req = signed_request(&unique_nonce(), now, "consumer-test-1")?;

    let resp = router.oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

/// Replaying the exact same request (same nonce) must be rejected with 401.
#[tokio::test]
#[ignore]
async fn test_replay_rejected_on_second_submission() -> Result<()> {
    let router = build_router(ReplayConfig::default()).await?;
    let now = chrono::Utc::now().timestamp();
    let nonce = unique_nonce();
    let consumer = format!("consumer-replay-{}", unique_nonce());

    let req1 = signed_request(&nonce, now, &consumer)?;
    let resp1 = router.clone().oneshot(req1).await?;
    assert_eq!(resp1.status(), StatusCode::OK, "first request should be accepted");

    let req2 = signed_request(&nonce, now, &consumer)?;
    let resp2 = router.oneshot(req2).await?;
    assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED, "replay should be rejected");

    let body = axum::body::to_bytes(resp2.into_body(), usize::MAX)
        .await
        .context("read response body")?;
    let json: serde_json::Value =
        serde_json::from_slice(&body).context("parse response JSON")?;
    assert_eq!(json["error"]["code"], "REPLAY_DETECTED");
    Ok(())
}

/// A timestamp older than the configured window must be rejected.
#[tokio::test]
#[ignore]
async fn test_timestamp_too_old_rejected() -> Result<()> {
    let router = build_router(ReplayConfig::default()).await?;
    let old_ts = chrono::Utc::now().timestamp() - 400; // 400 s old, window is 300 s
    let req = signed_request(&unique_nonce(), old_ts, "consumer-ts-old")?;

    let resp = router.oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .context("read response body")?;
    let json: serde_json::Value =
        serde_json::from_slice(&body).context("parse response JSON")?;
    assert_eq!(json["error"]["code"], "TIMESTAMP_TOO_OLD");
    Ok(())
}

/// A timestamp too far in the future must be rejected.
#[tokio::test]
#[ignore]
async fn test_timestamp_in_future_rejected() -> Result<()> {
    let router = build_router(ReplayConfig::default()).await?;
    let future_ts = chrono::Utc::now().timestamp() + 120; // 120 s ahead, tolerance is 30 s
    let req = signed_request(&unique_nonce(), future_ts, "consumer-ts-future")?;

    let resp = router.oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .context("read response body")?;
    let json: serde_json::Value =
        serde_json::from_slice(&body).context("parse response JSON")?;
    assert_eq!(json["error"]["code"], "TIMESTAMP_IN_FUTURE");
    Ok(())
}

/// A request at exactly the timestamp boundary (window_secs old) must be accepted.
#[tokio::test]
#[ignore]
async fn test_timestamp_at_exact_boundary_accepted() -> Result<()> {
    let cfg = ReplayConfig {
        timestamp_window_secs: 300,
        ..Default::default()
    };
    let router = build_router(cfg).await?;
    let boundary_ts = chrono::Utc::now().timestamp() - 300;
    let consumer = format!("consumer-boundary-{}", unique_nonce());
    let req = signed_request(&unique_nonce(), boundary_ts, &consumer)?;

    let resp = router.oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

/// Missing X-Aframp-Timestamp header must return 401.
#[tokio::test]
#[ignore]
async fn test_missing_timestamp_header_rejected() -> Result<()> {
    let router = build_router(ReplayConfig::default()).await?;
    let req = Request::builder()
        .method("POST")
        .uri("/transfer")
        .header("x-aframp-nonce", unique_nonce())
        .body(Body::empty())?;

    let resp = router.oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .context("read response body")?;
    let json: serde_json::Value =
        serde_json::from_slice(&body).context("parse response JSON")?;
    assert_eq!(json["error"]["code"], "MISSING_TIMESTAMP");
    Ok(())
}

/// Missing X-Aframp-Nonce header must return 401.
#[tokio::test]
#[ignore]
async fn test_missing_nonce_header_rejected() -> Result<()> {
    let router = build_router(ReplayConfig::default()).await?;
    let now = chrono::Utc::now().timestamp();
    let req = Request::builder()
        .method("POST")
        .uri("/transfer")
        .header("x-aframp-timestamp", now.to_string())
        .body(Body::empty())?;

    let resp = router.oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .context("read response body")?;
    let json: serde_json::Value =
        serde_json::from_slice(&body).context("parse response JSON")?;
    assert_eq!(json["error"]["code"], "MISSING_NONCE");
    Ok(())
}

/// Two concurrent requests with the same nonce — only one must succeed.
/// This verifies the atomic SET NX prevents race conditions.
#[tokio::test]
#[ignore]
async fn test_concurrent_duplicate_requests_only_one_accepted() -> Result<()> {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let pool = init_cache_pool(CacheConfig {
        redis_url,
        ..Default::default()
    })
    .await
    .context("Redis init failed")?;

    let state = ReplayPreventionState {
        redis: Arc::new(pool),
        config: Arc::new(ReplayConfig::default()),
    };

    let router = Router::new()
        .route("/transfer", post(|| async { "ok".into_response() }))
        .layer(middleware::from_fn_with_state(
            state,
            replay_prevention_middleware,
        ));

    let now = chrono::Utc::now().timestamp();
    let nonce = unique_nonce();
    let consumer = format!("consumer-concurrent-{}", unique_nonce());

    let (resp1, resp2) = tokio::join!(
        router.clone().oneshot(signed_request(&nonce, now, &consumer)?),
        router.clone().oneshot(signed_request(&nonce, now, &consumer)?),
    );

    let s1 = resp1.context("first request failed")?.status();
    let s2 = resp2.context("second request failed")?.status();

    let ok_count = [s1, s2]
        .iter()
        .filter(|&&s| s == StatusCode::OK)
        .count();
    let reject_count = [s1, s2]
        .iter()
        .filter(|&&s| s == StatusCode::UNAUTHORIZED)
        .count();

    assert_eq!(ok_count, 1, "exactly one concurrent request should succeed");
    assert_eq!(reject_count, 1, "exactly one concurrent request should be rejected");
    Ok(())
}

/// Different consumers may reuse the same nonce value without conflict.
#[tokio::test]
#[ignore]
async fn test_same_nonce_different_consumers_both_accepted() -> Result<()> {
    let router = build_router(ReplayConfig::default()).await?;
    let now = chrono::Utc::now().timestamp();
    let shared_nonce = unique_nonce();

    let req_a = signed_request(&shared_nonce, now, "consumer-alpha")?;
    let req_b = signed_request(&shared_nonce, now, "consumer-beta")?;

    let resp_a = router.clone().oneshot(req_a).await?;
    let resp_b = router.clone().oneshot(req_b).await?;

    assert_eq!(resp_a.status(), StatusCode::OK, "consumer-alpha should be accepted");
    assert_eq!(
        resp_b.status(),
        StatusCode::OK,
        "consumer-beta should be accepted with same nonce"
    );
    Ok(())
}
