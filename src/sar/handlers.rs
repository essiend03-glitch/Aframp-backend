//! SAR review queue HTTP handlers
//!
//! GET  /api/v1/sar/queue          — list SARs pending review
//! GET  /api/v1/sar/:id            — get a single SAR
//! POST /api/v1/sar/:id/approve    — compliance officer approves
//! POST /api/v1/sar/:id/reject     — compliance officer rejects
//! GET  /api/v1/sar/:id/audit      — full audit trail for a SAR

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use super::{models::ReviewRequest, service::SarService};

pub type SarState = Arc<SarService>;

pub async fn list_queue(State(svc): State<SarState>) -> impl IntoResponse {
    match svc.list_pending().await {
        Ok(reports) => (StatusCode::OK, Json(serde_json::json!({ "reports": reports }))).into_response(),
        Err(e) => err(e),
    }
}

pub async fn get_sar(
    State(svc): State<SarState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get(id).await {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "not_found" }))).into_response(),
        Err(e) => err(e),
    }
}

pub async fn approve_sar(
    State(svc): State<SarState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ReviewRequest>,
) -> impl IntoResponse {
    match svc
        .approve(id, &body.officer_id, body.notes.as_deref(), body.amended_report.as_deref())
        .await
    {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => err(e),
    }
}

pub async fn reject_sar(
    State(svc): State<SarState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ReviewRequest>,
) -> impl IntoResponse {
    match svc.reject(id, &body.officer_id, body.notes.as_deref()).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => err(e),
    }
}

pub async fn get_audit(
    State(svc): State<SarState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_audit_log(id).await {
        Ok(entries) => (StatusCode::OK, Json(serde_json::json!({ "audit": entries }))).into_response(),
        Err(e) => err(e),
    }
}

fn err(e: anyhow::Error) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}
