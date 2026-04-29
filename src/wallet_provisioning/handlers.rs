//! HTTP handlers for Wallet Provisioning endpoints (Issue #322).

use crate::wallet_provisioning::{models::*, service::WalletProvisioningService};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub type ProvisioningState = Arc<WalletProvisioningService>;

// ---------------------------------------------------------------------------
// Keypair guidance
// ---------------------------------------------------------------------------

/// GET /v1/wallet/keypair-guidance
pub async fn get_keypair_guidance(
    State(svc): State<ProvisioningState>,
) -> impl IntoResponse {
    let guidance = svc.get_keypair_guidance();
    (StatusCode::OK, Json(json!({ "data": guidance }))).into_response()
}

/// GET /v1/wallet/mnemonic-challenge
pub async fn get_mnemonic_challenge(
    State(svc): State<ProvisioningState>,
) -> impl IntoResponse {
    let challenge = svc.get_mnemonic_challenge();
    (StatusCode::OK, Json(json!({ "data": challenge }))).into_response()
}

// ---------------------------------------------------------------------------
// Funding requirements
// ---------------------------------------------------------------------------

/// GET /v1/wallet/:wallet_id/funding-requirements
pub async fn get_funding_requirements(
    State(svc): State<ProvisioningState>,
    Path(wallet_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_funding_requirements(wallet_id).await {
        Ok(req) => (StatusCode::OK, Json(json!({ "data": req }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Provisioning status
// ---------------------------------------------------------------------------

/// GET /v1/wallet/:wallet_id/provisioning-status
pub async fn get_provisioning_status(
    State(svc): State<ProvisioningState>,
    Path(wallet_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_provisioning_status(wallet_id).await {
        Ok(status) => (StatusCode::OK, Json(json!({ "data": status }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Trustline
// ---------------------------------------------------------------------------

/// POST /v1/wallet/:wallet_id/trustline/initiate
pub async fn initiate_trustline(
    State(svc): State<ProvisioningState>,
    Path(wallet_id): Path<Uuid>,
    Query(params): Query<WalletAddressQuery>,
) -> impl IntoResponse {
    match svc.initiate_trustline(wallet_id, &params.wallet_address).await {
        Ok(resp) => (StatusCode::OK, Json(json!({ "data": resp }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /v1/wallet/:wallet_id/trustline/submit
pub async fn submit_trustline(
    State(svc): State<ProvisioningState>,
    Path(wallet_id): Path<Uuid>,
    Query(params): Query<WalletAddressQuery>,
    Json(req): Json<TrustlineSubmitRequest>,
) -> impl IntoResponse {
    match svc.submit_trustline(wallet_id, &params.wallet_address, req).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "message": "Trustline submitted successfully" })),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Readiness
// ---------------------------------------------------------------------------

/// GET /v1/wallet/:wallet_id/readiness
pub async fn get_readiness(
    State(svc): State<ProvisioningState>,
    Path(wallet_id): Path<Uuid>,
    Query(params): Query<WalletAddressQuery>,
) -> impl IntoResponse {
    match svc.check_readiness(wallet_id, &params.wallet_address).await {
        Ok(resp) => (StatusCode::OK, Json(json!({ "data": resp }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Admin: funding account
// ---------------------------------------------------------------------------

/// GET /v1/admin/wallet/funding-account
pub async fn get_funding_account(
    State(svc): State<ProvisioningState>,
) -> impl IntoResponse {
    match svc.get_funding_account_status().await {
        Ok(status) => (StatusCode::OK, Json(json!({ "data": status }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// POST /v1/admin/wallet/funding-account/replenish
pub async fn replenish_funding_account(
    State(svc): State<ProvisioningState>,
    Query(actor): Query<ActorQuery>,
    Json(req): Json<ReplenishmentRequest>,
) -> impl IntoResponse {
    match svc.request_replenishment(actor.actor_user_id, req).await {
        Ok(_) => (
            StatusCode::CREATED,
            Json(json!({ "message": "Replenishment request submitted" })),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Shared query params
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct WalletAddressQuery {
    pub wallet_address: String,
}

#[derive(Deserialize)]
pub struct ActorQuery {
    pub actor_user_id: Option<Uuid>,
}
