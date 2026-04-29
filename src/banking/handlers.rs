//! Banking Integration — HTTP Handlers (Issue #407)

use super::models::{CreateMandateRequest, InitiateTransferRequest, LinkAccountRequest};
use super::service::BankingService;
use super::webhook::BankWebhookProcessor;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub type BankingState = Arc<BankingService>;
pub type WebhookState = Arc<BankWebhookProcessor>;

// ── Account Endpoints ─────────────────────────────────────────────────────────

pub async fn link_account(
    State(svc): State<BankingState>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<LinkAccountRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match svc.link_account(user_id, req).await {
        Ok(account) => Ok((StatusCode::CREATED, Json(json!({ "data": account })))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

pub async fn list_accounts(
    State(svc): State<BankingState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match svc.list_accounts(user_id).await {
        Ok(accounts) => Ok(Json(json!({ "data": accounts }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

pub async fn unlink_account(
    State(svc): State<BankingState>,
    Path((user_id, account_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match svc.unlink_account(account_id, user_id).await {
        Ok(()) => Ok(Json(json!({ "message": "Account unlinked" }))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

// ── Mandate Endpoints ─────────────────────────────────────────────────────────

pub async fn create_mandate(
    State(svc): State<BankingState>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<CreateMandateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match svc.create_mandate(user_id, req).await {
        Ok(mandate) => Ok((StatusCode::CREATED, Json(json!({ "data": mandate })))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

pub async fn revoke_mandate(
    State(svc): State<BankingState>,
    Path((user_id, mandate_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match svc.revoke_mandate(mandate_id, user_id).await {
        Ok(()) => Ok(Json(json!({ "message": "Mandate revoked" }))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

// ── Transfer Endpoints ────────────────────────────────────────────────────────

pub async fn initiate_transfer(
    State(svc): State<BankingState>,
    Json(req): Json<InitiateTransferRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match svc.initiate_transfer(req).await {
        Ok(transfer) => Ok((StatusCode::ACCEPTED, Json(json!({ "data": transfer })))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

// ── Webhook Endpoint ──────────────────────────────────────────────────────────

pub async fn receive_webhook(
    State(processor): State<WebhookState>,
    Path(provider): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match processor.process(&provider, &payload).await {
        Ok(()) => Ok(Json(json!({ "received": true }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}
