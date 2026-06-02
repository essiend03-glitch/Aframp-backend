//! HTTP handlers for commission management endpoints (Issue #471).

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    models::{CreateCommissionStructureInput, ManualAdjustmentInput},
    service::CommissionService,
};

#[derive(Clone)]
pub struct CommissionState {
    pub service: Arc<CommissionService>,
}

#[derive(Deserialize)]
pub struct StatementQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}
fn default_limit() -> i64 { 50 }

// POST /api/v1/admin/partners/commissions/configure
pub async fn configure_commission(
    State(s): State<Arc<CommissionState>>,
    Json(input): Json<CreateCommissionStructureInput>,
) -> impl IntoResponse {
    match s.service.configure_structure(input).await {
        Ok(structure) => (StatusCode::CREATED, Json(serde_json::json!(structure))).into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// GET /api/v1/partners/:partner_id/revenue/statement
pub async fn revenue_statement(
    State(s): State<Arc<CommissionState>>,
    Path(partner_id): Path<Uuid>,
    Query(q): Query<StatementQuery>,
) -> impl IntoResponse {
    match s.service.revenue_statement(partner_id, q.limit, q.offset).await {
        Ok(stmt) => Json(serde_json::json!(stmt)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// POST /api/v1/admin/partners/revenue/adjust
pub async fn manual_adjust(
    State(s): State<Arc<CommissionState>>,
    Json(input): Json<ManualAdjustmentInput>,
) -> impl IntoResponse {
    match s.service.manual_adjustment(input).await {
        Ok(entry) => (StatusCode::CREATED, Json(serde_json::json!(entry))).into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
