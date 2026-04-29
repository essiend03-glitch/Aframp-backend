//! HTTP handlers for CTR filing endpoints

use super::ctr_filing::{CtrDocuments, CtrFilingService, FilingResult};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

/// Shared state for CTR filing handlers
#[derive(Clone)]
pub struct CtrFilingState {
    pub service: Arc<CtrFilingService>,
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

/// POST /api/admin/compliance/ctrs/:ctr_id/generate
///
/// Generate CTR documents (NFIU-compliant XML and PDF)
pub async fn generate_documents(
    State(state): State<CtrFilingState>,
    Path(ctr_id): Path<Uuid>,
) -> Result<Json<ApiResponse<DocumentGenerationResponse>>, ApiError> {
    info!(ctr_id = %ctr_id, "Generating CTR documents");

    match state.service.generate_documents(ctr_id).await {
        Ok(documents) => {
            let response = DocumentGenerationResponse {
                ctr_id: documents.ctr_id,
                xml_size: documents.xml_content.len(),
                pdf_url: documents.pdf_url.clone(),
                generated_at: documents.generated_at,
                message: "CTR documents generated successfully".to_string(),
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, ctr_id = %ctr_id, "Failed to generate CTR documents");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// GET /api/admin/compliance/ctrs/:ctr_id/document
///
/// Get CTR document (XML or PDF)
pub async fn get_document(
    State(state): State<CtrFilingState>,
    Path(ctr_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    info!(ctr_id = %ctr_id, "Fetching CTR document");

    match state.service.get_document(ctr_id).await {
        Ok(Some(documents)) => {
            // Return XML content with appropriate headers
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/xml")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"ctr_{}.xml\"", ctr_id),
                )
                .body(documents.xml_content.into())
                .unwrap();

            Ok(response)
        }
        Ok(None) => Err(ApiError {
            success: false,
            error: "CTR document not found. Generate documents first.".to_string(),
        }),
        Err(e) => {
            error!(error = %e, ctr_id = %ctr_id, "Failed to fetch CTR document");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

/// POST /api/admin/compliance/ctrs/:ctr_id/file
///
/// File CTR with NFIU (with validation and retry logic)
pub async fn file_ctr(
    State(state): State<CtrFilingState>,
    Path(ctr_id): Path<Uuid>,
) -> Result<Json<ApiResponse<FilingResponse>>, ApiError> {
    info!(ctr_id = %ctr_id, "Filing CTR with NFIU");

    match state.service.file_ctr(ctr_id).await {
        Ok(result) => {
            let response = FilingResponse {
                ctr_id: result.ctr_id,
                filing_id: result.filing_id,
                submission_reference: result.submission_reference,
                submission_timestamp: result.submission_timestamp,
                status: format!("{:?}", result.status),
                retry_count: result.retry_count,
                message: "CTR filed successfully with NFIU".to_string(),
            };

            Ok(Json(ApiResponse {
                success: true,
                data: response,
            }))
        }
        Err(e) => {
            error!(error = %e, ctr_id = %ctr_id, "Failed to file CTR");
            Err(ApiError {
                success: false,
                error: e.to_string(),
            })
        }
    }
}

// Response types

#[derive(Debug, Serialize)]
pub struct DocumentGenerationResponse {
    pub ctr_id: Uuid,
    pub xml_size: usize,
    pub pdf_url: Option<String>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FilingResponse {
    pub ctr_id: Uuid,
    pub filing_id: Uuid,
    pub submission_reference: String,
    pub submission_timestamp: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub retry_count: u32,
    pub message: String,
}
