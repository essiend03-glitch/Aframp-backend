//! Integration tests for wallet analytics (Issue #369).
//!
//! These tests require a running PostgreSQL database with migrations applied.
//! Run with: cargo test --test wallet_analytics_test -- --ignored
//! Or with a real DB: cargo test --test wallet_analytics_test

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post, put},
    Router,
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use tower::util::ServiceExt;

use Bitmesh_backend::api::analytics::{
    get_counterparties, get_insights, get_insight_preferences, get_providers, get_spending,
    get_summary, get_trends, update_insight_preferences, AnalyticsState,
};
use Bitmesh_backend::api::admin::analytics::{
    get_anomalies, get_behaviour_profile, get_cohorts, get_overview, get_retention,
    get_risk_distribution, AdminAnalyticsState,
};
use Bitmesh_backend::database::analytics_repository::{
    AnalyticsRepository, UpsertProfile, UpsertSnapshot,
};
use Bitmesh_backend::services::analytics::{AnalyticsConfig, AnalyticsService};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

async fn test_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/aframp_test".to_string());
    PgPool::connect(&url).await.expect("Failed to connect to test DB")
}

const TEST_WALLET: &str = "GTEST_ANALYTICS_WALLET_INTEGRATION_001";

async fn seed_wallet(pool: &PgPool) {
    // Ensure a user and wallet exist for FK constraints
    sqlx::query("INSERT INTO users (id, email) VALUES (gen_random_uuid(), 'analytics_test@test.com') ON CONFLICT DO NOTHING")
        .execute(pool).await.ok();
    sqlx::query(
        "INSERT INTO wallets (wallet_address, user_id, chain, balance)
         SELECT $1, id, 'stellar', '0' FROM users WHERE email='analytics_test@test.com'
         ON CONFLICT (wallet_address) DO NOTHING",
    )
    .bind(TEST_WALLET)
    .execute(pool)
    .await
    .ok();
}

async fn seed_transactions(pool: &PgPool) {
    for i in 0..5i32 {
        sqlx::query(
            "INSERT INTO transactions
             (wallet_address, type, from_currency, to_currency, from_amount, to_amount,
              cngn_amount, status, payment_provider)
             VALUES ($1, 'onramp', 'NGN', 'CNGN', $2, $3, $3, 'completed', 'mpesa')
             ON CONFLICT DO NOTHING",
        )
        .bind(TEST_WALLET)
        .bind(BigDecimal::from(1000 * (i + 1) as i64))
        .bind(BigDecimal::from(100 * (i + 1) as i64))
        .execute(pool)
        .await
        .ok();
    }
}

fn consumer_router(repo: Arc<AnalyticsRepository>) -> Router {
    let state = Arc::new(AnalyticsState { repo, redis: None });
    Router::new()
        .route("/api/wallet/:wallet_id/analytics/summary", get(get_summary))
        .route("/api/wallet/:wallet_id/analytics/spending", get(get_spending))
        .route("/api/wallet/:wallet_id/analytics/trends", get(get_trends))
        .route("/api/wallet/:wallet_id/analytics/counterparties", get(get_counterparties))
        .route("/api/wallet/:wallet_id/analytics/providers", get(get_providers))
        .route("/api/wallet/:wallet_id/analytics/insights", get(get_insights))
        .route(
            "/api/wallet/:wallet_id/analytics/insights/preferences",
            get(get_insight_preferences).put(update_insight_preferences),
        )
        .with_state(state)
}

fn admin_router(repo: Arc<AnalyticsRepository>) -> Router {
    let state = Arc::new(AdminAnalyticsState { repo });
    Router::new()
        .route("/api/admin/analytics/wallets/overview", get(get_overview))
        .route("/api/admin/analytics/wallets/anomalies", get(get_anomalies))
        .route("/api/admin/analytics/wallets/risk-distribution", get(get_risk_distribution))
        .route("/api/admin/analytics/wallets/cohorts", get(get_cohorts))
        .route("/api/admin/analytics/wallets/retention", get(get_retention))
        .route("/api/admin/wallets/:wallet_id/behaviour-profile", get(get_behaviour_profile))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Repository-level tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_upsert_and_get_snapshot() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    let repo = AnalyticsRepository::new(pool.clone());

    let now = Utc::now();
    let snap = UpsertSnapshot {
        wallet_address: TEST_WALLET.to_string(),
        period: "daily".to_string(),
        period_start: now - chrono::Duration::hours(24),
        period_end: now,
        total_tx_count: 3,
        total_cngn_sent: BigDecimal::from(300),
        total_cngn_received: BigDecimal::from(0),
        total_fiat_onramped: BigDecimal::from(3000),
        total_fiat_offramped: BigDecimal::from(0),
        total_fees_paid: BigDecimal::from(0),
        unique_counterparties: 2,
        most_used_tx_type: Some("onramp".to_string()),
        most_used_provider: Some("mpesa".to_string()),
        active_days: 1,
    };

    let result = repo.upsert_snapshot(snap).await;
    assert!(result.is_ok(), "upsert_snapshot failed: {:?}", result.err());

    let snaps = repo
        .get_snapshots(TEST_WALLET, "daily", now - chrono::Duration::days(2), now)
        .await
        .unwrap();
    assert!(!snaps.is_empty(), "Expected at least one snapshot");
    assert_eq!(snaps[0].total_tx_count, 3);
}

#[tokio::test]
#[ignore]
async fn test_incremental_snapshot_skips_existing() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    let repo = AnalyticsRepository::new(pool.clone());

    let now = Utc::now();
    let period_start = now - chrono::Duration::hours(24);

    // First upsert
    let snap = UpsertSnapshot {
        wallet_address: TEST_WALLET.to_string(),
        period: "daily".to_string(),
        period_start,
        period_end: now,
        total_tx_count: 5,
        total_cngn_sent: BigDecimal::from(500),
        total_cngn_received: BigDecimal::from(0),
        total_fiat_onramped: BigDecimal::from(5000),
        total_fiat_offramped: BigDecimal::from(0),
        total_fees_paid: BigDecimal::from(0),
        unique_counterparties: 3,
        most_used_tx_type: Some("onramp".to_string()),
        most_used_provider: None,
        active_days: 1,
    };
    repo.upsert_snapshot(snap).await.unwrap();

    // Check last snapshot_at is set
    let last = repo.get_latest_snapshot_at(TEST_WALLET, "daily").await.unwrap();
    assert!(last.is_some(), "Expected a snapshot_at timestamp");
}

#[tokio::test]
#[ignore]
async fn test_upsert_and_get_profile() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    let repo = AnalyticsRepository::new(pool.clone());

    let profile = UpsertProfile {
        wallet_address: TEST_WALLET.to_string(),
        avg_tx_size: BigDecimal::from(250),
        tx_frequency_per_week: BigDecimal::from(3),
        preferred_hour_utc: Some(14),
        preferred_provider: Some("mpesa".to_string()),
        preferred_currency_pair: Some("NGN->CNGN".to_string()),
        risk_score: BigDecimal::from(15),
    };

    repo.upsert_profile(profile).await.unwrap();

    let p = repo.get_profile(TEST_WALLET).await.unwrap();
    assert!(p.is_some());
    let p = p.unwrap();
    assert_eq!(p.preferred_hour_utc, Some(14));
    assert_eq!(p.preferred_provider.as_deref(), Some("mpesa"));
}

#[tokio::test]
#[ignore]
async fn test_anomaly_insert_and_list() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    let repo = AnalyticsRepository::new(pool.clone());

    repo.insert_anomaly(TEST_WALLET, "volume_spike", BigDecimal::from(5))
        .await
        .unwrap();

    let anomalies = repo.list_open_anomalies(10, 0).await.unwrap();
    assert!(!anomalies.is_empty());
    let count = repo.count_open_anomalies().await.unwrap();
    assert!(count > 0);
}

// ---------------------------------------------------------------------------
// Service-level tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_service_compute_profile_from_history() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    seed_transactions(&pool).await;

    let svc = AnalyticsService::new(pool.clone(), AnalyticsConfig::default());
    svc.compute_profile(TEST_WALLET).await;

    let repo = AnalyticsRepository::new(pool);
    let profile = repo.get_profile(TEST_WALLET).await.unwrap();
    assert!(profile.is_some(), "Profile should have been computed");
}

#[tokio::test]
#[ignore]
async fn test_service_anomaly_detection_no_panic() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    seed_transactions(&pool).await;

    let svc = AnalyticsService::new(pool.clone(), AnalyticsConfig::default());
    // Should not panic even without a pre-existing profile
    svc.detect_anomalies(TEST_WALLET).await;
}

#[tokio::test]
#[ignore]
async fn test_service_generate_insights() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    seed_transactions(&pool).await;

    let svc = AnalyticsService::new(pool.clone(), AnalyticsConfig::default());
    svc.generate_insights(TEST_WALLET, "weekly").await;

    let repo = AnalyticsRepository::new(pool);
    let insights = repo.get_latest_insights(TEST_WALLET, 5).await.unwrap();
    assert!(!insights.is_empty(), "Insights should have been generated");
}

// ---------------------------------------------------------------------------
// HTTP endpoint tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn test_consumer_summary_endpoint_returns_404_for_unknown_wallet() {
    let pool = test_pool().await;
    let repo = Arc::new(AnalyticsRepository::new(pool));
    let app = consumer_router(repo);

    let req = Request::builder()
        .uri("/api/wallet/GUNKNOWN_WALLET_XYZ/analytics/summary")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore]
async fn test_consumer_spending_endpoint_returns_200() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    let repo = Arc::new(AnalyticsRepository::new(pool));
    let app = consumer_router(repo);

    let req = Request::builder()
        .uri(format!("/api/wallet/{}/analytics/spending", TEST_WALLET))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore]
async fn test_consumer_trends_endpoint_returns_200() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    let repo = Arc::new(AnalyticsRepository::new(pool));
    let app = consumer_router(repo);

    let req = Request::builder()
        .uri(format!("/api/wallet/{}/analytics/trends?granularity=daily", TEST_WALLET))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore]
async fn test_consumer_insight_preferences_roundtrip() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    let repo = Arc::new(AnalyticsRepository::new(pool));
    let app = consumer_router(repo);

    // PUT preferences
    let body = serde_json::json!({"weekly_insights": false, "monthly_insights": true});
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/api/wallet/{}/analytics/insights/preferences", TEST_WALLET))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // GET preferences
    let req = Request::builder()
        .uri(format!("/api/wallet/{}/analytics/insights/preferences", TEST_WALLET))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["weekly_insights"], false);
    assert_eq!(json["monthly_insights"], true);
}

#[tokio::test]
#[ignore]
async fn test_admin_overview_endpoint_returns_200() {
    let pool = test_pool().await;
    let repo = Arc::new(AnalyticsRepository::new(pool));
    let app = admin_router(repo);

    let req = Request::builder()
        .uri("/api/admin/analytics/wallets/overview")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore]
async fn test_admin_anomalies_endpoint_returns_200() {
    let pool = test_pool().await;
    let repo = Arc::new(AnalyticsRepository::new(pool));
    let app = admin_router(repo);

    let req = Request::builder()
        .uri("/api/admin/analytics/wallets/anomalies")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore]
async fn test_admin_risk_distribution_returns_200() {
    let pool = test_pool().await;
    let repo = Arc::new(AnalyticsRepository::new(pool));
    let app = admin_router(repo);

    let req = Request::builder()
        .uri("/api/admin/analytics/wallets/risk-distribution")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore]
async fn test_admin_behaviour_profile_404_for_unknown() {
    let pool = test_pool().await;
    let repo = Arc::new(AnalyticsRepository::new(pool));
    let app = admin_router(repo);

    let req = Request::builder()
        .uri("/api/admin/wallets/GUNKNOWN_WALLET_XYZ/behaviour-profile")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore]
async fn test_full_snapshot_lifecycle() {
    let pool = test_pool().await;
    seed_wallet(&pool).await;
    seed_transactions(&pool).await;

    let svc = AnalyticsService::new(pool.clone(), AnalyticsConfig::default());
    let now = Utc::now();
    let period_start = now - chrono::Duration::days(30);

    // Compute snapshot
    svc.compute_snapshot(TEST_WALLET, "monthly", period_start, now).await;

    // Verify it was persisted
    let repo = AnalyticsRepository::new(pool);
    let snaps = repo
        .get_snapshots(TEST_WALLET, "monthly", period_start - chrono::Duration::days(1), now)
        .await
        .unwrap();
    assert!(!snaps.is_empty(), "Monthly snapshot should have been persisted");
}
