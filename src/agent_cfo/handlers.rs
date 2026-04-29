use crate::agent_cfo::{
    engine::AgentCfoEngine,
    ledger::ExpenditureLedger,
    types::{CostProjectionRequest, LedgerQuery, RecordInferenceRequest},
    watchdog::BurnRateWatchdog,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct CfoState {
    pub engine: Arc<AgentCfoEngine>,
    pub ledger: Arc<ExpenditureLedger>,
    pub db: PgPool,
}

/// POST /agent-cfo/project-cost
///
/// Pre-execution cost projection. Returns whether the task fits within budget
/// and whether human approval is required.
pub async fn project_cost(
    State(s): State<CfoState>,
    Json(req): Json<CostProjectionRequest>,
) -> impl IntoResponse {
    match s.engine.project_cost(req).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /agent-cfo/inference
///
/// Record a single inference / API call event. Updates cumulative spend,
/// triggers graceful degradation and budget reports as needed.
pub async fn record_inference(
    State(s): State<CfoState>,
    Json(req): Json<RecordInferenceRequest>,
) -> impl IntoResponse {
    match s.engine.record_inference(req).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GET /agent-cfo/ledger
///
/// Query the expenditure ledger. Supports filtering by agent_id and task_id.
pub async fn query_ledger(
    State(s): State<CfoState>,
    Query(q): Query<LedgerQuery>,
) -> impl IntoResponse {
    match s.ledger.query(&q).await {
        Ok(events) => (StatusCode::OK, Json(events)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GET /agent-cfo/policy/:agent_id
///
/// Retrieve the budget policy for an agent.
pub async fn get_policy(
    State(s): State<CfoState>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.engine.get_policy(agent_id).await {
        Ok(policy) => (StatusCode::OK, Json(policy)).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /agent-cfo/unfreeze/:agent_id
///
/// Admin endpoint — unfreeze an agent's signing keys after manual review.
pub async fn unfreeze_agent(
    State(s): State<CfoState>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match BurnRateWatchdog::unfreeze(&s.db, agent_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "message": "Agent keys unfrozen", "agent_id": agent_id })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
