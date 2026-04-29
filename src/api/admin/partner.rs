//! Admin API for remittance partner management (Issue #408).
//!
//! POST   /api/admin/partners
//! GET    /api/admin/partners
//! GET    /api/admin/partners/:id
//! PATCH  /api/admin/partners/:id/status
//! PUT    /api/admin/partners/:id/branding
//! GET    /api/admin/partners/:id/branding
//! PUT    /api/admin/partners/:id/fees
//! GET    /api/admin/partners/:id/fees
//! PUT    /api/admin/partners/:id/limits
//! GET    /api/admin/partners/:id/limits
//! GET    /api/admin/partners/:id/settlements

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::database::partner_repository::PartnerRepository;
use crate::services::partner::PartnerService;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AdminPartnerState {
    pub repo: Arc<PartnerRepository>,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreatePartnerRequest {
    pub slug: String,
    pub name: String,
    pub api_key: String, // raw key — hashed before storage
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct UpsertBrandingRequest {
    pub logo_url: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub email_template: Option<serde_json::Value>,
    pub language_overrides: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertFeeRequest {
    pub corridor: String,
    pub fee_type: String,
    pub fee_value: String,
    pub min_amount: Option<String>,
    pub max_amount: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertLimitsRequest {
    pub daily_volume_limit: Option<String>,
    pub per_tx_min: String,
    pub per_tx_max: Option<String>,
    pub kyc_threshold: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bad_request(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response()
}
fn not_found(msg: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg}))).into_response()
}
fn internal_err(msg: &str) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response()
}

fn parse_bd(s: &str) -> Option<BigDecimal> {
    s.parse().ok()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn create_partner(
    State(state): State<Arc<AdminPartnerState>>,
    Json(body): Json<CreatePartnerRequest>,
) -> Response {
    let hash = PartnerService::hash_api_key(&body.api_key);
    match state.repo.create_partner(
        &body.slug,
        &body.name,
        &hash,
        body.webhook_url.as_deref(),
        body.webhook_secret.as_deref(),
    ).await {
        Ok(p) => (StatusCode::CREATED, Json(json!({
            "id": p.id, "slug": p.slug, "name": p.name, "status": p.status,
            "created_at": p.created_at,
        }))).into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn list_partners(
    State(state): State<Arc<AdminPartnerState>>,
) -> Response {
    match state.repo.list_partners().await {
        Ok(partners) => {
            let items: Vec<serde_json::Value> = partners.iter().map(|p| json!({
                "id": p.id, "slug": p.slug, "name": p.name, "status": p.status,
                "webhook_url": p.webhook_url, "created_at": p.created_at,
            })).collect();
            (StatusCode::OK, Json(json!({"partners": items}))).into_response()
        }
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn get_partner(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.repo.find_by_id(id).await {
        Ok(Some(p)) => (StatusCode::OK, Json(json!({
            "id": p.id, "slug": p.slug, "name": p.name, "status": p.status,
            "webhook_url": p.webhook_url, "created_at": p.created_at, "updated_at": p.updated_at,
        }))).into_response(),
        Ok(None) => not_found("Partner not found"),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn update_partner_status(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateStatusRequest>,
) -> Response {
    if !["active", "suspended", "pending"].contains(&body.status.as_str()) {
        return bad_request("status must be active, suspended, or pending");
    }
    match state.repo.update_partner_status(id, &body.status).await {
        Ok(_) => (StatusCode::OK, Json(json!({"status": body.status}))).into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn upsert_branding(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpsertBrandingRequest>,
) -> Response {
    match state.repo.upsert_branding(
        id,
        body.logo_url.as_deref(),
        body.primary_color.as_deref(),
        body.secondary_color.as_deref(),
        body.email_template.unwrap_or(json!({})),
        body.language_overrides.unwrap_or(json!({})),
    ).await {
        Ok(_) => (StatusCode::OK, Json(json!({"updated": true}))).into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn get_branding(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.repo.get_branding(id).await {
        Ok(Some(b)) => (StatusCode::OK, Json(json!({
            "logo_url": b.logo_url, "primary_color": b.primary_color,
            "secondary_color": b.secondary_color, "email_template": b.email_template,
            "language_overrides": b.language_overrides,
        }))).into_response(),
        Ok(None) => (StatusCode::OK, Json(json!({}))).into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn upsert_fee(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpsertFeeRequest>,
) -> Response {
    let fee_value = match parse_bd(&body.fee_value) {
        Some(v) => v,
        None => return bad_request("Invalid fee_value"),
    };
    if !["percent", "flat"].contains(&body.fee_type.as_str()) {
        return bad_request("fee_type must be percent or flat");
    }
    match state.repo.upsert_fee(
        id,
        &body.corridor,
        &body.fee_type,
        fee_value,
        body.min_amount.as_deref().and_then(parse_bd),
        body.max_amount.as_deref().and_then(parse_bd),
    ).await {
        Ok(_) => (StatusCode::OK, Json(json!({"updated": true}))).into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn list_fees(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.repo.list_fees(id).await {
        Ok(fees) => {
            let items: Vec<serde_json::Value> = fees.iter().map(|f| json!({
                "id": f.id, "corridor": f.corridor, "fee_type": f.fee_type,
                "fee_value": f.fee_value.to_string(), "is_active": f.is_active,
                "min_amount": f.min_amount.as_ref().map(|v| v.to_string()),
                "max_amount": f.max_amount.as_ref().map(|v| v.to_string()),
            })).collect();
            (StatusCode::OK, Json(json!({"fees": items}))).into_response()
        }
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn upsert_limits(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpsertLimitsRequest>,
) -> Response {
    let per_tx_min = match parse_bd(&body.per_tx_min) {
        Some(v) => v,
        None => return bad_request("Invalid per_tx_min"),
    };
    match state.repo.upsert_limits(
        id,
        body.daily_volume_limit.as_deref().and_then(parse_bd),
        per_tx_min,
        body.per_tx_max.as_deref().and_then(parse_bd),
        body.kyc_threshold.as_deref().and_then(parse_bd),
    ).await {
        Ok(_) => (StatusCode::OK, Json(json!({"updated": true}))).into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn get_limits(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.repo.get_limits(id).await {
        Ok(Some(l)) => (StatusCode::OK, Json(json!({
            "daily_volume_limit": l.daily_volume_limit.as_ref().map(|v| v.to_string()),
            "per_tx_min": l.per_tx_min.to_string(),
            "per_tx_max": l.per_tx_max.as_ref().map(|v| v.to_string()),
            "kyc_threshold": l.kyc_threshold.as_ref().map(|v| v.to_string()),
        }))).into_response(),
        Ok(None) => (StatusCode::OK, Json(json!({}))).into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn list_settlements(
    State(state): State<Arc<AdminPartnerState>>,
    Path(id): Path<Uuid>,
) -> Response {
    match state.repo.list_settlements(id, 90).await {
        Ok(settlements) => {
            let items: Vec<serde_json::Value> = settlements.iter().map(|s| json!({
                "id": s.id, "settlement_date": s.settlement_date.to_string(),
                "total_volume": s.total_volume.to_string(),
                "total_fees": s.total_fees.to_string(),
                "net_payable": s.net_payable.to_string(),
                "tx_count": s.tx_count, "status": s.status, "report_url": s.report_url,
            })).collect();
            (StatusCode::OK, Json(json!({"settlements": items}))).into_response()
        }
        Err(e) => internal_err(&e.to_string()),
    }
}
