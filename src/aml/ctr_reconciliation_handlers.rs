//! CTR Reconciliation API Handlers
//!
//! HTTP handlers for CTR reconciliation and monthly reporting endpoints.

use super::ctr_reconciliation::{CtrReconciliationService, ReconciliationRequest};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

/// State for reconciliation handlers
#[derive(Clone)]
pub struct CtrReconciliationState {
    pub reconciliation_service: Arc<CtrReconciliationService>,
}

/// POST /api/admin/compliance/ctrs/reconcile
///
/// Reconcile CTRs with transaction data for a date range
pub async fn reconcile_ctrs(
    State(state): State<CtrReconciliationState>,
    Json(request): Json<ReconciliationRequest>,
) -> impl IntoResponse {
    info!(
        start_date = %request.start_date,
        end_date = %request.end_date,
        "Reconciliation request received"
    );

    match state
        .reconciliation_service
        .reconcile(request)
        .await
    {
        Ok(result) => {
            info!(
                reconciliation_id = %result.reconciliation_id,
                total_ctrs = result.total_ctrs_checked,
                discrepancies = result.ctrs_with_discrepancies,
                "Reconciliation completed successfully"
            );
            (StatusCode::OK, Json(result)).into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to reconcile CTRs");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to reconcile CTRs",
                    "details": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

/// Monthly report request
#[derive(Debug, Deserialize)]
pub struct MonthlyReportRequest {
    pub year: i32,
    pub month: u32,
}

/// GET /api/admin/compliance/ctrs/monthly-report?year=2024&month=1
///
/// Generate monthly CTR activity report
pub async fn get_monthly_report(
    State(state): State<CtrReconciliationState>,
    axum::extract::Query(params): axum::extract::Query<MonthlyReportRequest>,
) -> impl IntoResponse {
    info!(
        year = params.year,
        month = params.month,
        "Monthly report request received"
    );

    match state
        .reconciliation_service
        .generate_monthly_report(params.year, params.month)
        .await
    {
        Ok(report) => {
            info!(
                report_id = %report.report_id,
                year = report.year,
                month = report.month,
                total_ctrs = report.total_ctrs_generated,
                "Monthly report generated successfully"
            );
            (StatusCode::OK, Json(report)).into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to generate monthly report");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to generate monthly report",
                    "details": e.to_string()
                })),
            )
                .into_response()
        }
    }
}
