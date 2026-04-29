//! HTTP handlers for CTR management endpoints

use super::ctr_management::{
    ApproveCtrRequest, CtrManagementService, CtrWithDetails, ReturnForCorrectionRequest,
    ReviewCtrRequest,
};
use super::models::{Ctr, CtrStatus};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

/// Shared state for CTR management handlers
#[derive(Clone)]
pub struct CtrManagementState {
    pub service: Arc<CtrManagementService>,
}

/// Standard API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub success: bool,
    pub error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::BAD_REQUEST;
        (status, Json(self)).into_response()
    }
}

/// Query parameters for listing CTRs
#[derive(Debug, Deserialize)]
pub struct ListCtrsQuery {
    pub status: Option<String>,
}

/// GET /api/admin/compliance/ctrs
///
/// Get all CTRs with optional status filter
pub async fn get_ctrs(
    State(state): State<CtrManagementState>,
    Query(query): Query<ListCtrsQuery>,
) -> Result<Json<ApiResponse<CtrsListResponse>>, ApiError> {
    info!("Fetching CTRs with filter: {:?}", query.status);

    // Parse status filter
    let status_filter = if let Some(status_str) = query.status {
        match status_str.to_lowercase().as_str() {
            "draft" => Some(CtrStatus::Draft),
            "under_review" | "under-review" => Some(CtrStatus::UnderReview),
            "approved" => Some(CtrStatus::Approved),
            "filed" => Some(CtrStatus::Filed),
            "acknowledged" => Some(CtrStatus::Acknowledged),
            "rejected" => Some(CtrStatus::Rejected),
            _ => {
                return Err(ApiError {
                    success: false,
                    error: format!("Invalid status filter: {}", status_str),
                })
            }
        }
    } else {
        None
    };

    match state.service.get_all_ctrs(status_filter).await {
        Ok(ctrs) => {
            let response = CtrsListResponse {
                ctrs,
                total_count: ctrs.len(),
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to fetch CTRs");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// GET /api/admin/compliance/ctrs/:ctr_id
///
/// Get CTR by ID with full details including transactions, reviews, and approvals
pub async fn get_ctr_by_id(
    State(state): State<CtrManagementState>,
    Path(ctr_id): Path<Uuid>,
) -> Result<Json<ApiResponse<CtrWithDetails>>, ApiError> {
    info!(ctr_id = %ctr_id, "Fetching CTR details");

    match state.service.get_ctr_with_details(ctr_id).await {
        Ok(Some(ctr_details)) => Ok(Json(ApiResponse {
            success: true,
            data: ctr_details,
        })),
        Ok(None) => Err(ApiError {
            success: false,
            error: "CTR not found".to_string(),
        }),
        Err(e) => {
            error!(error = %e, ctr_id = %ctr_id, "Failed to fetch CTR");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// POST /api/admin/compliance/ctrs/:ctr_id/review
///
/// Review a CTR with mandatory checklist
pub async fn review_ctr(
    State(state): State<CtrManagementState>,
    Path(ctr_id): Path<Uuid>,
    Json(request): Json<ReviewCtrRequest>,
) -> Result<Json<ApiResponse<ReviewResponse>>, ApiError> {
    info!(
        ctr_id = %ctr_id,
        reviewer_id = %request.reviewer_id,
        "Reviewing CTR"
    );

    match state.service.review_ctr(ctr_id, request).await {
        Ok(result) => {
            let response = ReviewResponse {
                ctr_id: result.ctr_id,
                review_id: result.review_id,
                checklist_complete: result.checklist_complete,
                incomplete_items: result.incomplete_items,
                can_proceed_to_approval: result.can_proceed_to_approval,
                message: if result.checklist_complete {
                    "CTR reviewed successfully. Ready for approval.".to_string()
                } else {
                    "CTR reviewed but checklist is incomplete.".to_string()
                },
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, ctr_id = %ctr_id, "Failed to review CTR");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// POST /api/admin/compliance/ctrs/:ctr_id/approve
///
/// Approve a CTR (requires senior approval for high-value CTRs)
pub async fn approve_ctr(
    State(state): State<CtrManagementState>,
    Path(ctr_id): Path<Uuid>,
    Json(request): Json<ApproveCtrRequest>,
) -> Result<Json<ApiResponse<ApprovalResponse>>, ApiError> {
    info!(
        ctr_id = %ctr_id,
        approver_id = %request.approver_id,
        approval_level = %request.approval_level,
        "Approving CTR"
    );

    match state.service.approve_ctr(ctr_id, request).await {
        Ok(result) => {
            let message = if result.can_proceed_to_filing {
                "CTR approved successfully. Ready for filing.".to_string()
            } else if result.requires_senior_approval && !result.senior_approval_received {
                "CTR approval recorded. Senior officer approval still required.".to_string()
            } else {
                "CTR approval recorded.".to_string()
            };

            let response = ApprovalResponse {
                ctr_id: result.ctr_id,
                approval_id: result.approval_id,
                requires_senior_approval: result.requires_senior_approval,
                senior_approval_received: result.senior_approval_received,
                can_proceed_to_filing: result.can_proceed_to_filing,
                message,
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, ctr_id = %ctr_id, "Failed to approve CTR");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// POST /api/admin/compliance/ctrs/:ctr_id/return-for-correction
///
/// Return a CTR for correction
pub async fn return_for_correction(
    State(state): State<CtrManagementState>,
    Path(ctr_id): Path<Uuid>,
    Json(request): Json<ReturnForCorrectionRequest>,
) -> Result<Json<ApiResponse<CorrectionResponse>>, ApiError> {
    info!(
        ctr_id = %ctr_id,
        reviewer_id = %request.reviewer_id,
        "Returning CTR for correction"
    );

    match state
        .service
        .return_for_correction(ctr_id, request.clone())
        .await
    {
        Ok(_) => {
            let response = CorrectionResponse {
                ctr_id,
                issues: request.issues,
                message: "CTR returned for correction. Status set to Draft.".to_string(),
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, ctr_id = %ctr_id, "Failed to return CTR for correction");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// GET /api/admin/compliance/ctrs/senior-approval-required
///
/// Get CTRs requiring senior approval
pub async fn get_ctrs_requiring_senior_approval(
    State(state): State<CtrManagementState>,
) -> Result<Json<ApiResponse<CtrsListResponse>>, ApiError> {
    info!("Fetching CTRs requiring senior approval");

    match state.service.get_ctrs_requiring_senior_approval().await {
        Ok(ctrs) => {
            let response = CtrsListResponse {
                ctrs,
                total_count: ctrs.len(),
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to fetch CTRs requiring senior approval");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

// Response types

#[derive(Debug, Serialize)]
pub struct CtrsListResponse {
    pub ctrs: Vec<Ctr>,
    pub total_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ReviewResponse {
    pub ctr_id: Uuid,
    pub review_id: Uuid,
    pub checklist_complete: bool,
    pub incomplete_items: Vec<String>,
    pub can_proceed_to_approval: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ApprovalResponse {
    pub ctr_id: Uuid,
    pub approval_id: Uuid,
    pub requires_senior_approval: bool,
    pub senior_approval_received: bool,
    pub can_proceed_to_filing: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CorrectionResponse {
    pub ctr_id: Uuid,
    pub issues: Vec<String>,
    pub message: String,
}
