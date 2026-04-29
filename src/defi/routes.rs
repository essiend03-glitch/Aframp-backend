use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, patch, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthMiddleware;
use crate::error::AppError;
use super::{
    DeFiService, CreateStrategyRequest, SubmitForApprovalRequest, GovernanceApprovalRequest,
    StrategyResponse, DeFiOverviewResponse, CircuitBreakerStatusResponse,
};

/// DeFi API routes
pub fn defi_routes(
    defi_service: Arc<DeFiService>,
) -> Router {
    Router::new()
        // Strategy Management
        .route("/strategies", post(create_strategy))
        .route("/strategies", get(list_strategies))
        .route("/strategies/:strategy_id", get(get_strategy))
        .route("/strategies/:strategy_id/submit-for-approval", post(submit_strategy_for_approval))
        .route("/strategies/:strategy_id/approve", post(approve_strategy))
        .route("/strategies/:strategy_id/reject", post(reject_strategy))
        .route("/strategies/:strategy_id/activate", post(activate_strategy))
        .route("/strategies/:strategy_id/pause", post(pause_strategy))
        .route("/strategies/:strategy_id/deprecate", post(deprecate_strategy))
        .route("/strategies/:strategy_id/allocations", get(get_strategy_allocations))
        .route("/strategies/:strategy_id/performance", get(get_strategy_performance))
        .route("/strategies/comparison", get(compare_strategies))
        .route("/strategies/recommendations", get(get_strategy_recommendations))
        
        // Circuit Breaker Management
        .route("/strategies/:strategy_id/circuit-breaker/status", get(get_circuit_breaker_status))
        .route("/strategies/:strategy_id/circuit-breaker/reset", post(reset_circuit_breaker))
        
        // Protocol Management
        .route("/protocols", get(list_protocols))
        .route("/protocols/:protocol_id/evaluate", post(evaluate_protocol))
        .route("/protocols/:protocol_id/approve", post(approve_protocol))
        
        // Treasury Management
        .route("/treasury/overview", get(get_treasury_overview))
        .route("/treasury/allocations", get(get_treasury_allocations))
        .route("/treasury/allocate", post(allocate_treasury_funds))
        .route("/treasury/withdraw", post(withdraw_treasury_funds))
        .route("/treasury/rebalance", post(rebalance_treasury))
        
        // General DeFi Overview
        .route("/overview", get(get_defi_overview))
        
        .layer(AuthMiddleware::new())
        .with_state(defi_service)
}

// ── Strategy Management Endpoints ─────────────────────────────────────────────

/// Create a new yield strategy
async fn create_strategy(
    State(defi_service): State<Arc<DeFiService>>,
    Json(request): Json<CreateStrategyRequest>,
    auth_user: AuthMiddleware,
) -> Result<Json<super::YieldStrategy>, AppError> {
    let strategy = defi_service.create_strategy(request, &auth_user.user_id).await?;
    Ok(Json(strategy))
}

/// List all strategies with optional filtering
async fn list_strategies(
    State(defi_service): State<Arc<DeFiService>>,
    Query(params): Query<ListStrategiesParams>,
) -> Result<Json<Vec<super::YieldStrategy>>, AppError> {
    let strategies = defi_service.list_strategies(
        params.status.map(|s| s.parse().unwrap()),
        params.limit,
        params.offset,
    ).await?;
    Ok(Json(strategies))
}

/// Get strategy details
async fn get_strategy(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
) -> Result<Json<StrategyResponse>, AppError> {
    let strategy = defi_service.get_strategy_details(strategy_id).await?;
    Ok(Json(strategy))
}

/// Submit strategy for governance approval
async fn submit_strategy_for_approval(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
    Json(_request): Json<SubmitForApprovalRequest>,
    auth_user: AuthMiddleware,
) -> Result<Json<super::GovernanceApprovalRecord>, AppError> {
    let approval = defi_service.submit_strategy_for_approval(strategy_id, &auth_user.user_id).await?;
    Ok(Json(approval))
}

/// Approve a strategy (committee member)
async fn approve_strategy(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
    Json(request): Json<GovernanceApprovalRequest>,
    auth_user: AuthMiddleware,
) -> Result<StatusCode, AppError> {
    defi_service.record_governance_vote(
        strategy_id,
        &auth_user.user_id,
        request.approval_type,
        &request.justification,
    ).await?;
    
    Ok(StatusCode::OK)
}

/// Reject a strategy (committee member)
async fn reject_strategy(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
    Json(request): Json<GovernanceApprovalRequest>,
    auth_user: AuthMiddleware,
) -> Result<StatusCode, AppError> {
    defi_service.record_governance_vote(
        strategy_id,
        &auth_user.user_id,
        request.approval_type,
        &request.justification,
    ).await?;
    
    Ok(StatusCode::OK)
}

/// Activate an approved strategy
async fn activate_strategy(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
) -> Result<Json<super::YieldStrategy>, AppError> {
    let strategy = defi_service.activate_strategy(strategy_id).await?;
    Ok(Json(strategy))
}

/// Pause an active strategy
async fn pause_strategy(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    defi_service.update_strategy_status(strategy_id, super::StrategyStatus::Paused).await?;
    Ok(StatusCode::OK)
}

/// Deprecate a strategy
async fn deprecate_strategy(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    defi_service.update_strategy_status(strategy_id, super::StrategyStatus::Deprecated).await?;
    Ok(StatusCode::OK)
}

/// Get strategy allocations
async fn get_strategy_allocations(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
) -> Result<Json<Vec<super::StrategyAllocation>>, AppError> {
    let allocations = defi_service.get_strategy_allocations(strategy_id).await?;
    Ok(Json(allocations))
}

/// Get strategy performance
async fn get_strategy_performance(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
    Query(params): Query<PerformanceParams>,
) -> Result<Json<Vec<super::StrategyPerformance>>, AppError> {
    let performance = defi_service.get_strategy_performance(strategy_id, params.limit).await?;
    Ok(Json(performance))
}

/// Compare strategies
async fn compare_strategies(
    State(defi_service): State<Arc<DeFiService>>,
) -> Result<Json<Vec<super::StrategyPerformanceSummary>>, AppError> {
    let comparison = defi_service.compare_strategies().await?;
    Ok(Json(comparison))
}

/// Get strategy recommendations
async fn get_strategy_recommendations(
    State(defi_service): State<Arc<DeFiService>>,
) -> Result<Json<Vec<super::StrategyRecommendation>>, AppError> {
    let recommendations = defi_service.get_strategy_recommendations().await?;
    Ok(Json(recommendations))
}

// ── Circuit Breaker Endpoints ─────────────────────────────────────────────────

/// Get circuit breaker status
async fn get_circuit_breaker_status(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
) -> Result<Json<CircuitBreakerStatusResponse>, AppError> {
    let status = defi_service.get_circuit_breaker_status(strategy_id).await?;
    Ok(Json(status))
}

/// Reset circuit breaker
async fn reset_circuit_breaker(
    State(defi_service): State<Arc<DeFiService>>,
    Path(strategy_id): Path<Uuid>,
    Json(request): Json<ResetCircuitBreakerRequest>,
    auth_user: AuthMiddleware,
) -> Result<StatusCode, AppError> {
    defi_service.reset_circuit_breaker(
        strategy_id,
        &auth_user.user_id,
        &request.reason,
    ).await?;
    
    Ok(StatusCode::OK)
}

// ── Protocol Management Endpoints ─────────────────────────────────────────────

/// List all DeFi protocols
async fn list_protocols(
    State(defi_service): State<Arc<DeFiService>>,
) -> Result<Json<Vec<super::ProtocolConfig>>, AppError> {
    let protocols = defi_service.list_protocols().await?;
    Ok(Json(protocols))
}

/// Evaluate a protocol
async fn evaluate_protocol(
    State(defi_service): State<Arc<DeFiService>>,
    Path(protocol_id): Path<String>,
) -> Result<Json<super::EvaluationResult>, AppError> {
    let evaluation = defi_service.evaluate_protocol(&protocol_id).await?;
    Ok(Json(evaluation))
}

/// Approve a protocol
async fn approve_protocol(
    State(defi_service): State<Arc<DeFiService>>,
    Path(protocol_id): Path<String>,
    Json(request): Json<ProtocolApprovalRequest>,
    auth_user: AuthMiddleware,
) -> Result<StatusCode, AppError> {
    defi_service.approve_protocol(&protocol_id, &auth_user.user_id, &request.justification).await?;
    Ok(StatusCode::OK)
}

// ── Treasury Management Endpoints ─────────────────────────────────────────────

/// Get treasury overview
async fn get_treasury_overview(
    State(defi_service): State<Arc<DeFiService>>,
) -> Result<Json<super::TreasuryExposureMetrics>, AppError> {
    let overview = defi_service.get_treasury_overview().await?;
    Ok(Json(overview))
}

/// Get treasury allocations
async fn get_treasury_allocations(
    State(defi_service): State<Arc<DeFiService>>,
) -> Result<Json<Vec<super::TreasuryAllocation>>, AppError> {
    let allocations = defi_service.get_treasury_allocations().await?;
    Ok(Json(allocations))
}

/// Allocate treasury funds
async fn allocate_treasury_funds(
    State(defi_service): State<Arc<DeFiService>>,
    Json(request): Json<TreasuryAllocationRequest>,
    auth_user: AuthMiddleware,
) -> Result<Json<super::TreasuryAllocation>, AppError> {
    let allocation = defi_service.allocate_to_protocol(
        &request.protocol_id,
        request.amount,
        request.allocation_type,
        &auth_user.user_id,
    ).await?;
    
    Ok(Json(allocation))
}

/// Withdraw treasury funds
async fn withdraw_treasury_funds(
    State(defi_service): State<Arc<DeFiService>>,
    Json(request): Json<TreasuryWithdrawalRequest>,
    auth_user: AuthMiddleware,
) -> Result<Json<super::TreasuryAllocation>, AppError> {
    let allocation = defi_service.withdraw_from_protocol(
        request.allocation_id,
        request.amount,
        &request.reason,
    ).await?;
    
    Ok(Json(allocation))
}

/// Rebalance treasury
async fn rebalance_treasury(
    State(defi_service): State<Arc<DeFiService>>,
    Json(request): Json<TreasuryRebalancingRequest>,
    auth_user: AuthMiddleware,
) -> Result<Json<Vec<super::TreasuryAllocation>>, AppError> {
    let allocations = defi_service.rebalance_treasury(
        request.target_allocations,
        &auth_user.user_id,
    ).await?;
    
    Ok(Json(allocations))
}

// ── General DeFi Overview ─────────────────────────────────────────────────────

/// Get comprehensive DeFi overview
async fn get_defi_overview(
    State(defi_service): State<Arc<DeFiService>>,
) -> Result<Json<DeFiOverviewResponse>, AppError> {
    let overview = defi_service.get_defi_overview().await?;
    Ok(Json(overview))
}

// ── Request/Response DTOs ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListStrategiesParams {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PerformanceParams {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ResetCircuitBreakerRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ProtocolApprovalRequest {
    pub justification: String,
}

#[derive(Debug, Deserialize)]
pub struct TreasuryAllocationRequest {
    pub protocol_id: String,
    pub amount: sqlx::types::BigDecimal,
    pub allocation_type: super::TreasuryAllocationType,
}

#[derive(Debug, Deserialize)]
pub struct TreasuryWithdrawalRequest {
    pub allocation_id: Uuid,
    pub amount: sqlx::types::BigDecimal,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct TreasuryRebalancingRequest {
    pub target_allocations: std::collections::HashMap<String, f64>,
}
