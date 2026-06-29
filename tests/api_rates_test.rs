//! Integration tests for the rates API endpoint
//!
//! Tests cover:
//! - Single pair queries
//! - Multiple pairs queries
//! - All pairs queries
//! - Caching behavior
//! - Error handling
//! - CORS headers
//! - ETag support

use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::Value;
use tower::ServiceExt;

use aframp_backend::api::rates::{get_rates, options_rates, RatesState};
use aframp_backend::database::connection::create_pool;
use aframp_backend::database::exchange_rate_repository::ExchangeRateRepository;
use aframp_backend::services::exchange_rate::{ExchangeRateService, ExchangeRateServiceConfig};
use std::sync::Arc;

async fn create_test_app() -> Router {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/aframp_test".to_string());

    // SAFETY: pool creation is a test precondition; panic is appropriate if the DB is unavailable.
    let db_pool = create_pool(&database_url)
        .await
        .expect("test precondition: database must be reachable at DATABASE_URL");

    let repository = ExchangeRateRepository::new(db_pool.clone());
    let config = ExchangeRateServiceConfig::default();
    let exchange_rate_service = ExchangeRateService::new(repository, config);

    let rates_state = RatesState {
        exchange_rate_service: Arc::new(exchange_rate_service),
        cache: None,
    };

    Router::new()
        .route("/api/rates", axum::routing::get(get_rates).options(options_rates))
        .with_state(rates_state)
}

/// Build a GET request for the given URI.
fn get(uri: &str) -> Result<Request<Body>> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .context("failed to build request")
}

/// Deserialize the response body as JSON.
async fn json_body(resp: axum::response::Response) -> Result<Value> {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .context("failed to read response body")?;
    serde_json::from_slice(&bytes).context("response body is not valid JSON")
}

#[tokio::test]
async fn test_single_pair_ngn_to_cngn() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=NGN&to=cNGN")?).await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::OK);

    let json = json_body(resp).await?;
    assert_eq!(json["pair"], "NGN/cNGN");
    assert_eq!(json["base_currency"], "NGN");
    assert_eq!(json["quote_currency"], "cNGN");
    assert_eq!(json["rate"], "1.0");
    assert_eq!(json["inverse_rate"], "1.0");
    assert_eq!(json["source"], "fixed_peg");
    assert!(json["timestamp"].is_string());
    Ok(())
}

#[tokio::test]
async fn test_single_pair_cngn_to_ngn() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=cNGN&to=NGN")?).await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::OK);

    let json = json_body(resp).await?;
    assert_eq!(json["pair"], "cNGN/NGN");
    assert_eq!(json["rate"], "1.0");
    Ok(())
}

#[tokio::test]
async fn test_invalid_currency() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=XYZ&to=cNGN")?).await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let json = json_body(resp).await?;
    assert_eq!(json["error"]["code"], "INVALID_CURRENCY");
    assert!(json["error"]["supported_currencies"].is_array());
    Ok(())
}

#[tokio::test]
async fn test_invalid_pair() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=NGN&to=BTC")?).await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let json = json_body(resp).await?;
    assert_eq!(json["error"]["code"], "INVALID_PAIR");
    assert!(json["error"]["supported_pairs"].is_array());
    Ok(())
}

#[tokio::test]
async fn test_multiple_pairs() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?pairs=NGN/cNGN,cNGN/NGN")?).await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::OK);

    let json = json_body(resp).await?;
    assert!(json["rates"].is_array());
    assert_eq!(
        json["rates"].as_array().map(|a| a.len()),
        Some(2),
        "expected 2 rates in response"
    );
    assert!(json["timestamp"].is_string());
    Ok(())
}

#[tokio::test]
async fn test_all_pairs() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates")?).await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::OK);

    let json = json_body(resp).await?;
    assert!(json["rates"].is_object());
    assert!(json["rates"]["NGN/cNGN"].is_object());
    assert!(json["rates"]["cNGN/NGN"].is_object());
    assert!(json["supported_currencies"].is_array());
    assert!(json["timestamp"].is_string());
    Ok(())
}

#[tokio::test]
async fn test_cache_headers() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=NGN&to=cNGN")?).await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::OK);

    let headers = resp.headers();
    assert!(headers.contains_key(header::CACHE_CONTROL));
    assert_eq!(
        headers.get(header::CACHE_CONTROL).and_then(|v| v.to_str().ok()),
        Some("public, max-age=30"),
    );
    assert!(headers.contains_key(header::ETAG));
    Ok(())
}

#[tokio::test]
async fn test_cors_headers() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=NGN&to=cNGN")?).await
        .context("oneshot failed")?;

    let headers = resp.headers();
    assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).and_then(|v| v.to_str().ok()),
        Some("*"),
    );
    assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_METHODS));
    Ok(())
}

#[tokio::test]
async fn test_options_preflight() -> Result<()> {
    let app = create_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/rates")
                .body(Body::empty())
                .context("failed to build OPTIONS request")?,
        )
        .await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let headers = resp.headers();
    assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_METHODS));
    Ok(())
}

#[tokio::test]
async fn test_response_format_single_pair() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=NGN&to=cNGN")?).await
        .context("oneshot failed")?;

    let json = json_body(resp).await?;
    assert!(json["pair"].is_string());
    assert!(json["base_currency"].is_string());
    assert!(json["quote_currency"].is_string());
    assert!(json["rate"].is_string());
    assert!(json["inverse_rate"].is_string());
    assert!(json["spread_percentage"].is_string());
    assert!(json["last_updated"].is_string());
    assert!(json["source"].is_string());
    assert!(json["timestamp"].is_string());
    Ok(())
}

#[tokio::test]
async fn test_inverse_rate_calculation() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=NGN&to=cNGN")?).await
        .context("oneshot failed")?;

    let json = json_body(resp).await?;
    assert_eq!(json["rate"], "1.0");
    assert_eq!(json["inverse_rate"], "1.0");
    Ok(())
}

#[tokio::test]
async fn test_missing_parameters() -> Result<()> {
    let app = create_test_app().await;
    let resp = app.oneshot(get("/api/rates?from=NGN")?).await
        .context("oneshot failed")?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let json = json_body(resp).await?;
    assert_eq!(json["error"]["code"], "INVALID_PARAMETERS");
    Ok(())
}
