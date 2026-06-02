//! PEP HTTP handlers

use super::models::{PepAuditAction, PepEddStatus, PepMatchStatus, PepScreeningRequest};
use super::extended_models::EnhancedPepScreeningResult;
use super::repository::PepRepository;
use super::screening::PepScreeningService;
use axum::{
    extract::{Path, Query, State},
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
        // Screening endpoints
        .route("/api/pep/screen", post(screen_consumer))
        .route("/api/pep/screen/enhanced", post(screen_consumer_enhanced))
        .route("/api/pep/matches/:consumer_id", get(get_matches))
        .route("/api/pep/matches/:match_id/review", put(review_match))
        
        // EDD case management
        .route("/api/pep/edd", get(list_open_edd_cases))
        .route("/api/pep/edd/:case_id", get(get_edd_case).put(update_edd_case))
        
        // Audit log
        .route("/api/pep/audit/:consumer_id", get(get_audit_log))
        
        // Admin: Database status
        .route("/api/admin/compliance/pep/database-status", get(get_database_status))
        
        // Admin: PEP Profile management
        .route("/api/admin/compliance/pep/profiles", get(list_pep_profiles))
        .route("/api/admin/compliance/pep/profiles/:pep_id", get(get_pep_profile))
        .route("/api/admin/compliance/pep/profiles/:pep_id/confirm", post(confirm_pep_profile))
        .route("/api/admin/compliance/pep/profiles/:pep_id/clear", post(clear_pep_profile))
        .route("/api/admin/compliance/pep/profiles/:pep_id/review", post(review_pep_profile))
        
        // Admin: Family members and associates
        .route("/api/admin/compliance/pep/profiles/:pep_id/family-members", post(add_family_member))
        .route("/api/admin/compliance/pep/profiles/:pep_id/associates", post(add_associate))
        
        // Admin: EDD workflow
        .route("/api/admin/compliance/pep/profiles/:pep_id/edd/initiate", post(initiate_edd))
        .route("/api/admin/compliance/pep/profiles/:pep_id/edd/complete", post(complete_edd))
        
        // Admin: Transaction monitoring
        .route("/api/admin/compliance/pep/profiles/:pep_id/transactions", get(get_pep_transactions))
        
        // Admin: Monitoring and metrics
        .route("/api/admin/compliance/pep/monitoring-status", get(get_monitoring_status))
        .route("/api/admin/compliance/pep/metrics", get(get_pep_metrics))
        
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
        full_name: body.full_name.clone(),
        date_of_birth: body.date_of_birth,
        nationality: body.nationality,
        country_of_residence: body.country_of_residence,
        is_rescreening: false,
    };
    let result = s.screening.screen(&req).await;
    (StatusCode::OK, Json(result))
}

// ---------------------------------------------------------------------------
// Enhanced screen a consumer (with DOB and nationality matching)
// ---------------------------------------------------------------------------

async fn screen_consumer_enhanced(
    State(s): State<PepState>,
    Json(body): Json<ScreenRequest>,
) -> impl IntoResponse {
    let req = PepScreeningRequest {
        consumer_id: body.consumer_id,
        full_name: body.full_name.clone(),
        date_of_birth: body.date_of_birth,
        nationality: body.nationality,
        country_of_residence: body.country_of_residence,
        is_rescreening: false,
    };
    let result = s.screening.screen_enhanced(&req).await;
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
// ============================================================================
// Extended PEP Profile Management Endpoints
// ============================================================================

use super::extended_models::{
    CreateAssociateRequest, CreateFamilyMemberRequest, EnhancedPepScreeningResult,
    InitiateEddRequest, CompleteEddRequest, ConfirmPepRequest, ClearPepRequest,
    ReviewTransactionRequest, ProfileReviewRequest, PepDatabaseStatusResponse,
    MonitoringStatusResponse, PepMetricsResponse, PepProfile, PepFamilyMember,
    PepCloseAssociate, PepEddRecord, PepTransactionMonitoring,
};

/// GET /api/admin/compliance/pep/database-status
async fn get_database_status(
    State(s): State<PepState>,
) -> impl IntoResponse {
    // In production, would get from database service
    let status = s.screening.get_database_status().await;
    (StatusCode::OK, Json(status))
}

/// GET /api/admin/compliance/pep/profiles — List all PEP profiles with filtering
async fn list_pep_profiles(
    State(s): State<PepState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // Extract filter parameters
    let category = params.get("category").cloned();
    let status = params.get("status").cloned();
    let edd_status = params.get("edd_status").cloned();
    let country = params.get("country").cloned();
    let page = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(20);

    match s.repo.list_pep_profiles(category, status, edd_status, country, page, limit).await {
        Ok(profiles) => (StatusCode::OK, Json(serde_json::json!({ "profiles": profiles }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// GET /api/admin/compliance/pep/profiles/:pep_id — Full PEP profile detail
async fn get_pep_profile(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.repo.get_pep_profile_detail(pep_id).await {
        Ok(Some(profile)) => (StatusCode::OK, Json(serde_json::json!(profile))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "PEP profile not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /api/admin/compliance/pep/profiles/:pep_id/confirm — Confirm a PEP match
async fn confirm_pep_profile(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
    Json(body): Json<ConfirmPepRequest>,
) -> impl IntoResponse {
    match s.repo.confirm_pep_profile(pep_id, body.reviewer_id, body.notes.as_deref()).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /api/admin/compliance/pep/profiles/:pep_id/clear — Clear as false positive
async fn clear_pep_profile(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
    Json(body): Json<ClearPepRequest>,
) -> impl IntoResponse {
    if body.justification.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Justification is required for clearing" })),
        );
    }

    match s.repo.clear_pep_profile(pep_id, body.reviewer_id, &body.justification).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /api/admin/compliance/pep/profiles/:pep_id/review — Record periodic review
async fn review_pep_profile(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
    Json(body): Json<ProfileReviewRequest>,
) -> impl IntoResponse {
    match s.repo.record_profile_review(
        pep_id,
        body.reviewer_id,
        &body.findings,
        &body.decision,
        body.next_review_date,
    ).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /api/admin/compliance/pep/profiles/:pep_id/family-members — Link family member
async fn add_family_member(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
    Json(body): Json<CreateFamilyMemberRequest>,
) -> impl IntoResponse {
    let relationship = super::models::RelationshipType::from_str(&body.relationship_type);
    
    match s.repo.add_family_member(pep_id, body.family_member_kyc_id, relationship).await {
        Ok(member_id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "family_member_id": member_id })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /api/admin/compliance/pep/profiles/:pep_id/associates — Link close associate
async fn add_associate(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
    Json(body): Json<CreateAssociateRequest>,
) -> impl IntoResponse {
    let association = super::models::AssociationType::from_str(&body.association_type);
    
    match s.repo.add_associate(pep_id, body.associate_kyc_id, association).await {
        Ok(associate_id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "associate_id": associate_id })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /api/admin/compliance/pep/profiles/:pep_id/edd/initiate — Initiate EDD
async fn initiate_edd(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
    Json(body): Json<InitiateEddRequest>,
) -> impl IntoResponse {
    let edd_type = body.edd_type
        .as_ref()
        .map(|t| super::models::EddType::from_str(t))
        .unwrap_or(super::models::EddType::Standard);
    
    match s.repo.initiate_edd(pep_id, edd_type, body.assigned_officer).await {
        Ok(edd_id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "edd_id": edd_id, "status": "initiated" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /api/admin/compliance/pep/profiles/:pep_id/edd/complete — Complete EDD
async fn complete_edd(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
    Json(body): Json<CompleteEddRequest>,
) -> impl IntoResponse {
    if body.findings.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Findings are required" })),
        );
    }

    match s.repo.complete_edd(
        pep_id,
        body.officer_id,
        &body.findings,
        &body.approval_status,
        body.notes.as_deref(),
    ).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true, "status": "completed" }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// GET /api/admin/compliance/pep/profiles/:pep_id/transactions — Transaction history
async fn get_pep_transactions(
    State(s): State<PepState>,
    Path(pep_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.repo.get_pep_transactions(pep_id).await {
        Ok(transactions) => (
            StatusCode::OK,
            Json(serde_json::json!({ "transactions": transactions })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// GET /api/admin/compliance/pep/monitoring-status — Re-screening job status
async fn get_monitoring_status(
    State(s): State<PepState>,
) -> impl IntoResponse {
    match s.repo.get_monitoring_status().await {
        Ok(status) => (StatusCode::OK, Json(status)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// GET /api/admin/compliance/pep/metrics — PEP metrics
async fn get_pep_metrics(
    State(s): State<PepState>,
) -> impl IntoResponse {
    match s.repo.get_pep_metrics().await {
        Ok(metrics) => (StatusCode::OK, Json(metrics)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}