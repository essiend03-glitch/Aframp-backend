//! HTTP handlers for DR/BCP endpoints (Issue #DR-BCP).

use crate::dr_bcp::{models::*, service::DrBcpService};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub type DrBcpState = Arc<DrBcpService>;

/// GET /dr/status
pub async fn get_dr_status(State(svc): State<DrBcpState>) -> impl IntoResponse {
    match svc.get_status().await {
        Ok(status) => (StatusCode::OK, Json(json!(status))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /dr/incidents
pub async fn declare_incident(
    State(svc): State<DrBcpState>,
    Json(req): Json<DeclareDrIncidentRequest>,
) -> impl IntoResponse {
    match svc.declare_incident(req).await {
        Ok(incident) => (StatusCode::CREATED, Json(json!(incident))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

/// PATCH /dr/incidents/:id/status
pub async fn update_incident_status(
    State(svc): State<DrBcpState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateIncidentStatusRequest>,
) -> impl IntoResponse {
    match svc.update_incident_status(id, req).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /dr/incidents/:id/notify-regulator
pub async fn notify_regulator(
    State(svc): State<DrBcpState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SendRegulatoryNotificationRequest>,
) -> impl IntoResponse {
    match svc.send_regulatory_notification(id, req).await {
        Ok(notif) => (StatusCode::CREATED, Json(json!(notif))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /dr/restore-tests  (called by the CI restore-verification pipeline)
pub async fn record_restore_test(
    State(svc): State<DrBcpState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let backup_id = match body
        .get("backup_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "valid backup_id required" })),
            )
                .into_response()
        }
    };

    let result: RestoreTestResult = match body
        .get("result")
        .and_then(|v| v.as_str())
    {
        Some("passed") => RestoreTestResult::Passed,
        Some("partial") => RestoreTestResult::Partial,
        _ => RestoreTestResult::Failed,
    };

    let duration = body
        .get("restore_duration_seconds")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let rpo = body.get("rpo_achieved_seconds").and_then(|v| v.as_i64());
    let rto = body.get("rto_achieved_seconds").and_then(|v| v.as_i64());
    let err_msg = body
        .get("error_message")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match svc
        .record_restore_test(backup_id, result, duration, rpo, rto, err_msg)
        .await
    {
        Ok(run) => (StatusCode::CREATED, Json(json!(run))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}
