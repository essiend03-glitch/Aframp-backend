//! HTTP handlers for CTR batch filing and deadline monitoring endpoints

use super::ctr_batch_filing::{
    BatchFilingRequest, BatchFilingSummary, CtrBatchFilingService, DeadlineStatusReport,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use tracing::{error, info};

/// Shared state for batch filing handlers
#[derive(Clone)]
pub struct CtrBatchFilingState {
    pub service: Arc<CtrBatchFilingService>,
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

/// POST /api/admin/compliance/ctrs/batch-file
///
/// Batch file multiple CTRs with per-CTR status tracking
pub async fn batch_file_ctrs(
    State(state): State<CtrBatchFilingState>,
    Json(request): Json<BatchFilingRequest>,
) -> Result<Json<ApiResponse<BatchFilingSummary>>, ApiError> {
    info!(
        ctr_count = request.ctr_ids.len(),
        "Batch filing CTRs"
    );

    if request.ctr_ids.is_empty() {
        return Err(ApiError {
            success: false,
            error: "No CTR IDs provided".to_string(),
        });
    }

    match state.service.batch_file(request).await {
        Ok(summary) => Ok(Json(ApiResponse {
            success: true,
            data: summary,
        })),
        Err(e) => {
            error!(error = %e, "Batch filing failed");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// GET /api/admin/compliance/ctrs/deadline-status
///
/// Get deadline status for all pending CTRs
pub async fn get_deadline_status(
    State(state): State<CtrBatchFilingState>,
) -> Result<Json<ApiResponse<DeadlineStatusReport>>, ApiError> {
    info!("Fetching deadline status");

    match state.service.get_deadline_status().await {
        Ok(report) => Ok(Json(ApiResponse {
            success: true,
            data: report,
        })),
        Err(e) => {
            error!(error = %e, "Failed to get deadline status");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}
