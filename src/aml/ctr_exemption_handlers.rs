//! HTTP handlers for CTR exemption management endpoints

use super::ctr_exemption::{CtrExemptionService, CreateExemptionRequest, ExemptionWithStatus};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

/// Shared state for exemption handlers
#[derive(Clone)]
pub struct CtrExemptionState {
    pub service: Arc<CtrExemptionService>,
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

/// POST /api/admin/compliance/ctr/exemptions
///
/// Create a new CTR exemption for a subject
pub async fn create_exemption(
    State(state): State<CtrExemptionState>,
    Json(request): Json<CreateExemptionRequest>,
) -> Result<Json<ApiResponse<ExemptionResponse>>, ApiError> {
    info!(
        subject_id = %request.subject_id,
        category = %request.exemption_category,
        "Creating CTR exemption"
    );

    match state.service.create_exemption(request).await {
        Ok(exemption) => {
            let response = ExemptionResponse {
                subject_id: exemption.subject_id,
                exemption_category: exemption.exemption_category,
                exemption_basis: exemption.exemption_basis,
                expiry_date: exemption.expiry_date,
                message: "Exemption created successfully".to_string(),
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to create exemption");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// GET /api/admin/compliance/ctr/exemptions
///
/// Get all CTR exemptions with status information
pub async fn get_exemptions(
    State(state): State<CtrExemptionState>,
) -> Result<Json<ApiResponse<ExemptionsListResponse>>, ApiError> {
    info!("Fetching all CTR exemptions");

    match state.service.get_all_exemptions().await {
        Ok(exemptions) => {
            let response = ExemptionsListResponse {
                exemptions,
                total_count: exemptions.len(),
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to fetch exemptions");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// DELETE /api/admin/compliance/ctr/exemptions/:exemption_id
///
/// Delete a CTR exemption by subject ID
pub async fn delete_exemption(
    State(state): State<CtrExemptionState>,
    Path(exemption_id): Path<Uuid>,
) -> Result<Json<ApiResponse<DeleteResponse>>, ApiError> {
    info!(
        exemption_id = %exemption_id,
        "Deleting CTR exemption"
    );

    match state.service.delete_exemption(exemption_id).await {
        Ok(deleted) => {
            if deleted {
                Ok(Json(ApiResponse {
                    success: true,
                    data: DeleteResponse {
                        subject_id: exemption_id,
                        message: "Exemption deleted successfully".to_string(),
                    },
                }))
            } else {
                Err(ApiError {
                    success: false,
                    error: "Exemption not found".to_string(),
                })
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to delete exemption");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// GET /api/admin/compliance/ctr/exemptions/expiring
///
/// Get exemptions that are expiring soon
pub async fn get_expiring_exemptions(
    State(state): State<CtrExemptionState>,
) -> Result<Json<ApiResponse<ExemptionsListResponse>>, ApiError> {
    info!("Fetching expiring CTR exemptions");

    match state.service.get_expiring_exemptions().await {
        Ok(exemptions) => {
            let response = ExemptionsListResponse {
                exemptions,
                total_count: exemptions.len(),
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, "Failed to fetch expiring exemptions");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

// Response types

#[derive(Debug, Serialize)]
pub struct ExemptionResponse {
    pub subject_id: Uuid,
    pub exemption_category: String,
    pub exemption_basis: String,
    pub expiry_date: Option<chrono::DateTime<chrono::Utc>>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ExemptionsListResponse {
    pub exemptions: Vec<ExemptionWithStatus>,
    pub total_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub subject_id: Uuid,
    pub message: String,
}
