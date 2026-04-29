//! PEP HTTP handlers

use super::models::{PepAuditAction, PepEddStatus, PepMatchStatus, PepScreeningRequest};
use super::repository::PepRepository;
use super::screening::PepScreeningService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct PepState {
    pub screening: Arc<PepScreeningService>,
    pub repo: Arc<PepRepository>,
}

pub fn pep_routes(state: PepState) -> Router {
    Router::new()
        .route("/api/pep/screen", post(screen_consumer))
        .route("/api/pep/matches/:consumer_id", get(get_matches))
        .route("/api/pep/matches/:match_id/review", put(review_match))
        .route("/api/pep/edd", get(list_open_edd_cases))
        .route("/api/pep/edd/:case_id", get(get_edd_case).put(update_edd_case))
        .route("/api/pep/audit/:consumer_id", get(get_audit_log))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Screen a consumer (called during KYC onboarding)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ScreenRequest {
    pub consumer_id: Uuid,
    pub full_name: String,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub nationality: Option<String>,
    pub country_of_residence: Option<String>,
}

async fn screen_consumer(
    State(s): State<PepState>,
    Json(body): Json<ScreenRequest>,
) -> impl IntoResponse {
    let req = PepScreeningRequest {
        consumer_id: body.consumer_id,
        full_name: body.full_name,
        date_of_birth: body.date_of_birth,
        nationality: body.nationality,
        country_of_residence: body.country_of_residence,
        is_rescreening: false,
    };
    let result = s.screening.screen(&req).await;
    (StatusCode::OK, Json(result))
}

// ---------------------------------------------------------------------------
// Get all PEP matches for a consumer
// ---------------------------------------------------------------------------

async fn get_matches(
    State(s): State<PepState>,
    Path(consumer_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.repo.fetch_matches_for_consumer(consumer_id).await {
        Ok(matches) => (StatusCode::OK, Json(serde_json::json!({ "matches": matches }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

// ---------------------------------------------------------------------------
// Compliance officer reviews a match (confirm / dismiss)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ReviewMatchRequest {
    pub reviewer_id: Uuid,
    pub status: String, // "confirmed" | "false_positive"
    pub notes: Option<String>,
}

async fn review_match(
    State(s): State<PepState>,
    Path(match_id): Path<Uuid>,
    Json(body): Json<ReviewMatchRequest>,
) -> impl IntoResponse {
    let status = match body.status.as_str() {
        "confirmed" => PepMatchStatus::Confirmed,
        "false_positive" => PepMatchStatus::FalsePositive,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid status; use 'confirmed' or 'false_positive'" })),
            )
        }
    };

    // Fetch consumer_id for the audit log
    let consumer_id = match s.repo.get_consumer_id_for_match(match_id).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "match not found" })),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    if let Err(e) = s
        .repo
        .update_match_status(match_id, status.clone(), body.reviewer_id, body.notes.as_deref())
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let action = match status {
        PepMatchStatus::Confirmed => PepAuditAction::MatchConfirmed,
        PepMatchStatus::FalsePositive => PepAuditAction::MatchDismissed,
        _ => PepAuditAction::StatusChanged,
    };

    let _ = s
        .repo
        .append_audit_entry(
            consumer_id,
            action,
            Some(body.reviewer_id),
            serde_json::json!({ "match_id": match_id, "notes": body.notes }),
        )
        .await;

    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// EDD case management
// ---------------------------------------------------------------------------

async fn list_open_edd_cases(State(s): State<PepState>) -> impl IntoResponse {
    match s.repo.list_open_edd_cases().await {
        Ok(cases) => (StatusCode::OK, Json(serde_json::json!({ "cases": cases }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

async fn get_edd_case(
    State(s): State<PepState>,
    Path(case_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.repo.get_edd_case(case_id).await {
        Ok(Some(case)) => (StatusCode::OK, Json(serde_json::json!(case))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "EDD case not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
pub struct UpdateEddRequest {
    pub actor_id: Uuid,
    pub status: String,
    pub source_of_wealth_notes: Option<String>,
    pub source_of_funds_notes: Option<String>,
}

async fn update_edd_case(
    State(s): State<PepState>,
    Path(case_id): Path<Uuid>,
    Json(body): Json<UpdateEddRequest>,
) -> impl IntoResponse {
    let status = match body.status.as_str() {
        "in_progress" => PepEddStatus::InProgress,
        "pending_signoff" => PepEddStatus::PendingSignoff,
        "approved" => PepEddStatus::Approved,
        "rejected" => PepEddStatus::Rejected,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid status" })),
            )
        }
    };

    // Fetch consumer_id for audit log
    let consumer_id = match s.repo.get_edd_case(case_id).await {
        Ok(Some(c)) => c.consumer_id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "EDD case not found" })),
            )
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    };

    if let Err(e) = s
        .repo
        .update_edd_case(
            case_id,
            status.clone(),
            body.actor_id,
            body.source_of_wealth_notes.as_deref(),
            body.source_of_funds_notes.as_deref(),
        )
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        );
    }

    let action = match status {
        PepEddStatus::Approved => PepAuditAction::SeniorSignoffGranted,
        PepEddStatus::Rejected => PepAuditAction::SeniorSignoffDenied,
        _ => PepAuditAction::EddCaseUpdated,
    };

    let _ = s
        .repo
        .append_audit_entry(
            consumer_id,
            action,
            Some(body.actor_id),
            serde_json::json!({ "case_id": case_id }),
        )
        .await;

    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Tamper-proof audit log
// ---------------------------------------------------------------------------

async fn get_audit_log(
    State(s): State<PepState>,
    Path(consumer_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.repo.get_audit_log(consumer_id).await {
        Ok(entries) => (StatusCode::OK, Json(serde_json::json!({ "entries": entries }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}
