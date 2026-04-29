//! HTTP handlers for Merchant Invoicing endpoints.

use crate::merchant_invoicing::{
    models::*,
    service::MerchantInvoicingService,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub type InvoicingState = Arc<MerchantInvoicingService>;

// ---------------------------------------------------------------------------
// Tax rules
// ---------------------------------------------------------------------------

/// POST /v1/merchant/:merchant_id/invoicing/tax-rules
pub async fn create_tax_rule(
    State(svc): State<InvoicingState>,
    Path(merchant_id): Path<Uuid>,
    Json(req): Json<CreateTaxRuleRequest>,
) -> impl IntoResponse {
    match svc.create_tax_rule(merchant_id, req).await {
        Ok(rule) => (StatusCode::CREATED, Json(json!({ "data": rule }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/merchant/:merchant_id/invoicing/tax-rules
pub async fn list_tax_rules(
    State(svc): State<InvoicingState>,
    Path(merchant_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.list_tax_rules(merchant_id).await {
        Ok(rules) => (StatusCode::OK, Json(json!({ "data": rules }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Invoices
// ---------------------------------------------------------------------------

/// POST /v1/merchant/:merchant_id/invoicing/invoices
pub async fn create_invoice(
    State(svc): State<InvoicingState>,
    Path(merchant_id): Path<Uuid>,
    Json(req): Json<CreateInvoiceRequest>,
) -> impl IntoResponse {
    match svc.create_invoice(merchant_id, req).await {
        Ok(inv) => (StatusCode::CREATED, Json(json!({ "data": inv }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/merchant/:merchant_id/invoicing/invoices
pub async fn list_invoices(
    State(svc): State<InvoicingState>,
    Path(merchant_id): Path<Uuid>,
    Query(query): Query<InvoiceListQuery>,
) -> impl IntoResponse {
    match svc.list_invoices(merchant_id, &query).await {
        Ok(list) => (StatusCode::OK, Json(json!({ "data": list }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/merchant/:merchant_id/invoicing/invoices/:invoice_id
pub async fn get_invoice(
    State(svc): State<InvoicingState>,
    Path((merchant_id, invoice_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    match svc.get_invoice(merchant_id, invoice_id).await {
        Ok(inv) => (StatusCode::OK, Json(json!({ "data": inv }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Tax preview
// ---------------------------------------------------------------------------

/// POST /v1/merchant/:merchant_id/invoicing/tax-preview
pub async fn preview_tax(
    State(svc): State<InvoicingState>,
    Path(merchant_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let line_items: Vec<LineItem> = match serde_json::from_value(
        body.get("line_items").cloned().unwrap_or_default(),
    ) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Invalid line_items: {}", e) })),
            )
                .into_response()
        }
    };
    let region = body
        .get("region")
        .and_then(|v| v.as_str())
        .unwrap_or("NG")
        .to_string();

    match svc.preview_tax(merchant_id, line_items, &region).await {
        Ok(result) => (StatusCode::OK, Json(json!({ "data": result }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Tax reports
// ---------------------------------------------------------------------------

/// POST /v1/merchant/:merchant_id/invoicing/tax-reports
pub async fn generate_tax_report(
    State(svc): State<InvoicingState>,
    Path(merchant_id): Path<Uuid>,
    Json(req): Json<GenerateTaxReportRequest>,
) -> impl IntoResponse {
    match svc.generate_tax_report(merchant_id, req).await {
        Ok(report) => (StatusCode::CREATED, Json(json!({ "data": report }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/merchant/:merchant_id/invoicing/tax-reports
pub async fn list_tax_reports(
    State(svc): State<InvoicingState>,
    Path(merchant_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.list_tax_reports(merchant_id).await {
        Ok(reports) => (StatusCode::OK, Json(json!({ "data": reports }))).into_response(),
        Err(e) => e.into_response(),
    }
}
