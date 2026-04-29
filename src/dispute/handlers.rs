//! HTTP handlers for Merchant Dispute Resolution endpoints (Issue #337).

use crate::dispute::{
    models::*,
    service::DisputeService,
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

pub type DisputeState = Arc<DisputeService>;

// ---------------------------------------------------------------------------
// Customer endpoints
// ---------------------------------------------------------------------------

/// POST /v1/disputes
///
/// Customer opens a new dispute from their transaction history.
pub async fn open_dispute(
    State(svc): State<DisputeState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // In production, customer_wallet and merchant_id would come from the JWT
    // claims. We extract them from the request body for this implementation.
    let customer_wallet = match body.get("customer_wallet").and_then(|v| v.as_str()) {
        Some(w) => w.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "customer_wallet required" })),
            )
                .into_response()
        }
    };
    let merchant_id: Uuid = match body
        .get("merchant_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "valid merchant_id required" })),
            )
                .into_response()
        }
    };
    let transaction_amount: f64 = match body
        .get("transaction_amount")
        .and_then(|v| v.as_f64())
    {
        Some(a) => a,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "transaction_amount required" })),
            )
                .into_response()
        }
    };

    let req: OpenDisputeRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    match svc
        .open_dispute(&customer_wallet, merchant_id, transaction_amount, req)
        .await
    {
        Ok(dispute) => (StatusCode::CREATED, Json(json!({ "data": dispute }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/disputes/customer/:wallet
///
/// Customer views their own disputes.
pub async fn list_customer_disputes(
    State(svc): State<DisputeState>,
    Path(wallet): Path<String>,
    Query(query): Query<DisputeListQuery>,
) -> impl IntoResponse {
    match svc.list_customer_disputes(&wallet, &query).await {
        Ok(page) => (StatusCode::OK, Json(json!({ "data": page }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /v1/disputes/:id/evidence/customer
///
/// Customer submits evidence (photos, communication logs, etc.).
pub async fn submit_customer_evidence(
    State(svc): State<DisputeState>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let customer_wallet = match body.get("customer_wallet").and_then(|v| v.as_str()) {
        Some(w) => w.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "customer_wallet required" })),
            )
                .into_response()
        }
    };
    let req: SubmitEvidenceRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    match svc.submit_customer_evidence(id, &customer_wallet, req).await {
        Ok(ev) => (StatusCode::CREATED, Json(json!({ "data": ev }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Merchant endpoints
// ---------------------------------------------------------------------------

/// GET /v1/disputes/merchant/:merchant_id
///
/// Merchant views disputes filed against them.
pub async fn list_merchant_disputes(
    State(svc): State<DisputeState>,
    Path(merchant_id): Path<Uuid>,
    Query(query): Query<DisputeListQuery>,
) -> impl IntoResponse {
    match svc.list_merchant_disputes(merchant_id, &query).await {
        Ok(page) => (StatusCode::OK, Json(json!({ "data": page }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /v1/disputes/:id/respond
///
/// Merchant responds to a dispute and optionally offers a settlement.
pub async fn merchant_respond(
    State(svc): State<DisputeState>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let merchant_id: Uuid = match body
        .get("merchant_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "valid merchant_id required" })),
            )
                .into_response()
        }
    };
    let req: MerchantResponseRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    match svc.merchant_respond(id, merchant_id, req).await {
        Ok(dispute) => (StatusCode::OK, Json(json!({ "data": dispute }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /v1/disputes/:id/evidence/merchant
///
/// Merchant submits evidence.
pub async fn submit_merchant_evidence(
    State(svc): State<DisputeState>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let merchant_id: Uuid = match body
        .get("merchant_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "valid merchant_id required" })),
            )
                .into_response()
        }
    };
    let req: SubmitEvidenceRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    match svc.submit_merchant_evidence(id, merchant_id, req).await {
        Ok(ev) => (StatusCode::CREATED, Json(json!({ "data": ev }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Platform mediation endpoints
// ---------------------------------------------------------------------------

/// POST /v1/disputes/:id/resolve
///
/// Platform mediator issues the final decision and triggers the refund.
pub async fn resolve_dispute(
    State(svc): State<DisputeState>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mediator_id = body
        .get("mediator_id")
        .and_then(|v| v.as_str())
        .unwrap_or("platform")
        .to_string();

    let req: ResolveDisputeRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    match svc.resolve_dispute(id, &mediator_id, req).await {
        Ok(dispute) => (StatusCode::OK, Json(json!({ "data": dispute }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Shared read endpoints
// ---------------------------------------------------------------------------

/// GET /v1/disputes/:id
pub async fn get_dispute(
    State(svc): State<DisputeState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_dispute(id).await {
        Ok(Some(d)) => (StatusCode::OK, Json(json!({ "data": d }))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Dispute not found" })),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/disputes/:id/evidence
pub async fn list_evidence(
    State(svc): State<DisputeState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.list_evidence(id).await {
        Ok(ev) => (StatusCode::OK, Json(json!({ "data": ev }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/disputes/:id/audit
pub async fn get_audit_log(
    State(svc): State<DisputeState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_audit_log(id).await {
        Ok(log) => (StatusCode::OK, Json(json!({ "data": log }))).into_response(),
        Err(e) => e.into_response(),
    }
}
