use crate::regulatory_evidence::{
    models::*,
    service::{EvidenceError, RegulatoryEvidenceService},
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct RegulatoryEvidenceState {
    pub service: Arc<RegulatoryEvidenceService>,
}

// ── Error helper ──────────────────────────────────────────────────────────────

fn err(e: EvidenceError) -> Response {
    let status = e.status_code();
    (status, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
}

fn extract_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .unwrap_or("127.0.0.1")
        .trim()
        .to_string()
}

// ── Evidence packages ─────────────────────────────────────────────────────────

/// POST /api/v1/regulatory-evidence/packages
/// Generate a new evidence package for a given scope/period.
pub async fn generate_package(
    State(state): State<Arc<RegulatoryEvidenceState>>,
    headers: HeaderMap,
    Json(body): Json<GenerateEvidencePackageRequest>,
) -> Response {
    let ip = extract_ip(&headers);
    match state.service.generate_package(&body, &ip).await {
        Ok(pkg) => (StatusCode::CREATED, Json(serde_json::json!({ "data": pkg }))).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/v1/regulatory-evidence/packages
pub async fn list_packages(
    State(state): State<Arc<RegulatoryEvidenceState>>,
    Query(q): Query<EvidencePackageListQuery>,
) -> Response {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0);
    match state.service.list_packages(q.scope_label.as_deref(), limit, offset).await {
        Ok(pkgs) => (StatusCode::OK, Json(serde_json::json!({ "data": pkgs }))).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/v1/regulatory-evidence/packages/:id
pub async fn get_package(
    State(state): State<Arc<RegulatoryEvidenceState>>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.service.get_package(id).await {
        Ok(Some(pkg)) => (StatusCode::OK, Json(serde_json::json!({ "data": pkg }))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
        Err(e) => err(e),
    }
}

// ── Policy history ────────────────────────────────────────────────────────────

/// POST /api/v1/regulatory-evidence/policies
pub async fn record_policy_snapshot(
    State(state): State<Arc<RegulatoryEvidenceState>>,
    Json(body): Json<CreatePolicySnapshotRequest>,
) -> Response {
    match state.service.record_policy_snapshot(&body).await {
        Ok(snap) => (StatusCode::CREATED, Json(serde_json::json!({ "data": snap }))).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/v1/regulatory-evidence/policies/point-in-time
pub async fn policy_at_point_in_time(
    State(state): State<Arc<RegulatoryEvidenceState>>,
    Query(q): Query<PolicyAtPointInTimeQuery>,
) -> Response {
    match state.service.policy_at_point_in_time(&q).await {
        Ok(Some(snap)) => (StatusCode::OK, Json(serde_json::json!({ "data": snap }))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "No policy found at that point in time" }))).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/v1/regulatory-evidence/policies/:name/history
pub async fn list_policy_history(
    State(state): State<Arc<RegulatoryEvidenceState>>,
    Path(name): Path<String>,
) -> Response {
    match state.service.list_policy_history(&name).await {
        Ok(history) => (StatusCode::OK, Json(serde_json::json!({ "data": history }))).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/v1/regulatory-evidence/policies
pub async fn list_policy_names(
    State(state): State<Arc<RegulatoryEvidenceState>>,
) -> Response {
    match state.service.list_policy_names().await {
        Ok(names) => (StatusCode::OK, Json(serde_json::json!({ "data": names }))).into_response(),
        Err(e) => err(e),
    }
}

// ── System test reports ───────────────────────────────────────────────────────

/// POST /api/v1/regulatory-evidence/test-reports
pub async fn record_test_report(
    State(state): State<Arc<RegulatoryEvidenceState>>,
    Json(body): Json<CreateSystemTestReportRequest>,
) -> Response {
    match state.service.record_test_report(&body).await {
        Ok(report) => (StatusCode::CREATED, Json(serde_json::json!({ "data": report }))).into_response(),
        Err(e) => err(e),
    }
}

/// GET /api/v1/regulatory-evidence/test-reports
#[derive(Deserialize)]
pub struct TestReportQuery {
    pub report_type: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

pub async fn list_test_reports(
    State(state): State<Arc<RegulatoryEvidenceState>>,
    Query(q): Query<TestReportQuery>,
) -> Response {
    let limit = q.limit.unwrap_or(50).min(200);
    match state.service.list_test_reports(q.report_type.as_deref(), q.from, q.to, limit).await {
        Ok(reports) => (StatusCode::OK, Json(serde_json::json!({ "data": reports }))).into_response(),
        Err(e) => err(e),
    }
}
