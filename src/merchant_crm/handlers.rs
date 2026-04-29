//! HTTP handlers for Merchant CRM endpoints.

use crate::merchant_crm::{
    models::*,
    service::MerchantCrmService,
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

pub type CrmState = Arc<MerchantCrmService>;

// ---------------------------------------------------------------------------
// Customer profiles
// ---------------------------------------------------------------------------

/// POST /v1/merchant/:merchant_id/crm/customers
pub async fn opt_in_customer(
    State(svc): State<CrmState>,
    Path(merchant_id): Path<Uuid>,
    Json(req): Json<UpsertCustomerProfileRequest>,
) -> impl IntoResponse {
    match svc.opt_in_customer(merchant_id, req).await {
        Ok(profile) => (StatusCode::OK, Json(json!({ "data": profile }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/merchant/:merchant_id/crm/customers
pub async fn list_customers(
    State(svc): State<CrmState>,
    Path(merchant_id): Path<Uuid>,
    Query(query): Query<CustomerListQuery>,
) -> impl IntoResponse {
    match svc.list_customers(merchant_id, &query).await {
        Ok(list) => (StatusCode::OK, Json(json!({ "data": list }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/merchant/:merchant_id/crm/customers/:wallet_address
pub async fn get_customer(
    State(svc): State<CrmState>,
    Path((merchant_id, wallet_address)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    match svc.get_customer(merchant_id, &wallet_address, true).await {
        Ok(profile) => (StatusCode::OK, Json(json!({ "data": profile }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// PATCH /v1/merchant/:merchant_id/crm/customers/:wallet_address/tags
pub async fn update_tags(
    State(svc): State<CrmState>,
    Path((merchant_id, wallet_address)): Path<(Uuid, String)>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let tags: Vec<String> = body
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    match svc.update_tags(merchant_id, &wallet_address, tags).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "message": "Tags updated" }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Retention metrics
// ---------------------------------------------------------------------------

/// GET /v1/merchant/:merchant_id/crm/retention
pub async fn get_retention(
    State(svc): State<CrmState>,
    Path(merchant_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_retention_metrics(merchant_id).await {
        Ok(metrics) => (StatusCode::OK, Json(json!({ "data": metrics }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Segments
// ---------------------------------------------------------------------------

/// POST /v1/merchant/:merchant_id/crm/segments
pub async fn create_segment(
    State(svc): State<CrmState>,
    Path(merchant_id): Path<Uuid>,
    Json(req): Json<UpsertSegmentRequest>,
) -> impl IntoResponse {
    match svc.upsert_segment(merchant_id, req).await {
        Ok(seg) => (StatusCode::CREATED, Json(json!({ "data": seg }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/merchant/:merchant_id/crm/segments
pub async fn list_segments(
    State(svc): State<CrmState>,
    Path(merchant_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.list_segments(merchant_id).await {
        Ok(segs) => (StatusCode::OK, Json(json!({ "data": segs }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Privacy-first export
// ---------------------------------------------------------------------------

/// GET /v1/merchant/:merchant_id/crm/export/anonymised
pub async fn export_anonymised(
    State(svc): State<CrmState>,
    Path(merchant_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.export_anonymised(merchant_id).await {
        Ok(data) => (StatusCode::OK, Json(json!({ "data": data }))).into_response(),
        Err(e) => e.into_response(),
    }
}
