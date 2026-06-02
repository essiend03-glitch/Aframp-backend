use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthMiddleware;
use crate::error::AppError;
use super::models::*;
use super::service::DefiAnalyticsService;

pub fn defi_analytics_routes(svc: Arc<DefiAnalyticsService>) -> Router {
    Router::new()
        // Admin — platform
        .route("/api/admin/defi/analytics/platform-summary", get(platform_summary))
        .route("/api/admin/defi/analytics/platform-history", get(platform_history))
        // Admin — strategies
        .route("/api/admin/defi/analytics/strategies", get(list_strategies_analytics))
        .route("/api/admin/defi/analytics/strategies/:strategy_id", get(get_strategy_analytics))
        .route("/api/admin/defi/analytics/strategies/:strategy_id/attribution", get(get_yield_attribution))
        // Admin — protocols
        .route("/api/admin/defi/analytics/protocols", get(list_protocols_analytics))
        .route("/api/admin/defi/analytics/protocols/:protocol_id", get(get_protocol_analytics))
        // Admin — AMM
        .route("/api/admin/defi/analytics/amm/pools", get(list_amm_pools_analytics))
        .route("/api/admin/defi/analytics/amm/pools/:pool_id", get(get_amm_pool_analytics))
        // Admin — lending
        .route("/api/admin/defi/analytics/lending", get(get_lending_analytics))
        .route("/api/admin/defi/analytics/lending/liquidations", get(get_liquidation_analytics))
        // Admin — reports
        .route("/api/admin/defi/analytics/reports", get(list_reports))
        .route("/api/admin/defi/analytics/reports/:report_type/generate", post(generate_report))
        // Admin — export
        .route("/api/admin/defi/analytics/export", post(platform_export))
        // User-facing
        .route("/api/defi/analytics/summary", get(user_summary))
        .route("/api/defi/analytics/savings", get(user_savings_analytics))
        .route("/api/defi/analytics/lending", get(user_lending_analytics))
        .route("/api/defi/analytics/yield-history", get(user_yield_history))
        .route("/api/defi/analytics/export", post(user_export))
        .layer(AuthMiddleware::new())
        .with_state(svc)
}

// ── Admin handlers ────────────────────────────────────────────────────────────

async fn platform_summary(
    State(svc): State<Arc<DefiAnalyticsService>>,
) -> Result<Json<PlatformSummaryResponse>, AppError> {
    Ok(Json(svc.get_platform_summary().await?))
}

async fn platform_history(
    State(svc): State<Arc<DefiAnalyticsService>>,
    Query(p): Query<HistoryParams>,
) -> Result<Json<Vec<DefiPlatformSnapshot>>, AppError> {
    let limit = p.limit.unwrap_or(30).min(365);
    Ok(Json(svc.get_platform_history(limit).await?))
}

async fn list_strategies_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
) -> Result<Json<Vec<DefiStrategySnapshot>>, AppError> {
    Ok(Json(svc.get_all_strategies_analytics().await?))
}

async fn get_strategy_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
    Path(strategy_id): Path<Uuid>,
    Query(p): Query<PeriodParams>,
) -> Result<Json<Vec<DefiStrategySnapshot>>, AppError> {
    let limit = p.limit.unwrap_or(30).min(365);
    Ok(Json(svc.get_strategy_analytics(strategy_id, limit).await?))
}

async fn get_yield_attribution(
    State(svc): State<Arc<DefiAnalyticsService>>,
    Path(strategy_id): Path<Uuid>,
) -> Result<Json<YieldAttributionResponse>, AppError> {
    Ok(Json(svc.get_yield_attribution(strategy_id).await?))
}

async fn list_protocols_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
) -> Result<Json<Vec<DefiProtocolSnapshot>>, AppError> {
    Ok(Json(svc.get_all_protocols_analytics().await?))
}

async fn get_protocol_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
    Path(protocol_id): Path<String>,
    Query(p): Query<PeriodParams>,
) -> Result<Json<Vec<DefiProtocolSnapshot>>, AppError> {
    let limit = p.limit.unwrap_or(30).min(365);
    Ok(Json(svc.get_protocol_analytics(&protocol_id, limit).await?))
}

async fn list_amm_pools_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
) -> Result<Json<Vec<DefiAmmPoolSnapshot>>, AppError> {
    Ok(Json(svc.get_all_amm_pools_analytics().await?))
}

async fn get_amm_pool_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
    Path(pool_id): Path<String>,
    Query(p): Query<PeriodParams>,
) -> Result<Json<Vec<DefiAmmPoolSnapshot>>, AppError> {
    let limit = p.limit.unwrap_or(30).min(365);
    Ok(Json(svc.get_amm_pool_analytics(&pool_id, limit).await?))
}

async fn get_lending_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
) -> Result<Json<Option<DefiLendingSnapshot>>, AppError> {
    Ok(Json(svc.get_lending_analytics().await?))
}

async fn get_liquidation_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
) -> Result<Json<Vec<DefiLendingSnapshot>>, AppError> {
    Ok(Json(svc.get_lending_liquidation_analytics().await?))
}

async fn list_reports(
    State(svc): State<Arc<DefiAnalyticsService>>,
) -> Result<Json<Vec<DefiAnalyticsReport>>, AppError> {
    Ok(Json(svc.list_reports().await?))
}

async fn generate_report(
    State(svc): State<Arc<DefiAnalyticsService>>,
    Path(report_type): Path<String>,
) -> Result<Json<DefiAnalyticsReport>, AppError> {
    Ok(Json(svc.generate_report(&report_type).await?))
}

async fn platform_export(
    State(svc): State<Arc<DefiAnalyticsService>>,
    auth_user: AuthMiddleware,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, AppError> {
    Ok(Json(svc.request_platform_export(&auth_user.user_id, req).await?))
}

// ── User-facing handlers ──────────────────────────────────────────────────────

async fn user_summary(
    State(svc): State<Arc<DefiAnalyticsService>>,
    auth_user: AuthMiddleware,
) -> Result<Json<DefiUserSnapshot>, AppError> {
    let wallet_id = parse_wallet_id(&auth_user.user_id)?;
    Ok(Json(svc.get_user_summary(wallet_id).await?))
}

async fn user_savings_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
    auth_user: AuthMiddleware,
) -> Result<Json<DefiUserSnapshot>, AppError> {
    let wallet_id = parse_wallet_id(&auth_user.user_id)?;
    Ok(Json(svc.get_user_summary(wallet_id).await?))
}

async fn user_lending_analytics(
    State(svc): State<Arc<DefiAnalyticsService>>,
    auth_user: AuthMiddleware,
) -> Result<Json<DefiUserSnapshot>, AppError> {
    let wallet_id = parse_wallet_id(&auth_user.user_id)?;
    Ok(Json(svc.get_user_summary(wallet_id).await?))
}

async fn user_yield_history(
    State(svc): State<Arc<DefiAnalyticsService>>,
    auth_user: AuthMiddleware,
    Query(p): Query<PeriodParams>,
) -> Result<Json<Vec<DefiUserSnapshot>>, AppError> {
    let wallet_id = parse_wallet_id(&auth_user.user_id)?;
    let limit = p.limit.unwrap_or(30).min(365);
    Ok(Json(svc.get_user_yield_history(wallet_id, limit).await?))
}

async fn user_export(
    State(svc): State<Arc<DefiAnalyticsService>>,
    auth_user: AuthMiddleware,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, AppError> {
    let wallet_id = parse_wallet_id(&auth_user.user_id)?;
    Ok(Json(svc.request_user_export(wallet_id, req).await?))
}

fn parse_wallet_id(user_id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(user_id).map_err(|_| AppError::BadRequest("Invalid user ID format".into()))
}
