//! Admin HTTP handlers for Stellar Ecosystem Partner Integration (Issue #470).

#[cfg(feature = "database")]
use crate::stellar_ecosystem::{
    models::*,
    service::EcosystemService,
};
#[cfg(feature = "database")]
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
#[cfg(feature = "database")]
use std::sync::Arc;
#[cfg(feature = "database")]
use tracing::instrument;
#[cfg(feature = "database")]
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/v1/admin/ecosystem/stellar/anchors
// ─────────────────────────────────────────────────────────────────────────────

/// List all active anchor connections with trustline status and transaction volumes.
#[cfg(feature = "database")]
#[instrument(skip(svc))]
pub async fn list_anchors_handler(
    State(svc): State<Arc<EcosystemService>>,
) -> impl IntoResponse {
    match svc.list_anchors().await {
        Ok(anchors) => (StatusCode::OK, Json(serde_json::json!({ "anchors": anchors }))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to list anchors");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/v1/admin/ecosystem/stellar/anchors
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[instrument(skip(svc, req))]
pub async fn register_anchor_handler(
    State(svc): State<Arc<EcosystemService>>,
    Json(req): Json<CreateAnchorConnectionRequest>,
) -> impl IntoResponse {
    match svc.register_anchor(req).await {
        Ok(anchor) => (StatusCode::CREATED, Json(anchor)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to register anchor");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/v1/admin/ecosystem/stellar/dex/configure
// ─────────────────────────────────────────────────────────────────────────────

/// Update DEX configuration: slippage thresholds, liquidity requirements, asset pairs.
#[cfg(feature = "database")]
#[instrument(skip(svc, req))]
pub async fn configure_dex_handler(
    State(svc): State<Arc<EcosystemService>>,
    Json(req): Json<UpdateDexConfigRequest>,
) -> impl IntoResponse {
    svc.update_config(req).await;
    let cfg = svc.config().await;
    tracing::info!(
        max_slippage = %cfg.max_slippage,
        min_liquidity = %cfg.min_liquidity_depth,
        "DEX configuration updated"
    );
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "max_slippage": cfg.max_slippage,
            "min_liquidity_depth": cfg.min_liquidity_depth,
            "monitored_pairs": cfg.monitored_pairs,
        })),
    )
        .into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/v1/admin/ecosystem/stellar/transfers
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[instrument(skip(svc, req))]
pub async fn initiate_transfer_handler(
    State(svc): State<Arc<EcosystemService>>,
    Json(req): Json<InitiateTransferRequest>,
) -> impl IntoResponse {
    match svc.initiate_transfer(req).await {
        Ok(transfer) => (StatusCode::CREATED, Json(transfer)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Transfer initiation failed");
            let status = if e.to_string().contains("Slippage") {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/v1/admin/ecosystem/stellar/dex/path
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[instrument(skip(svc, req))]
pub async fn find_path_handler(
    State(svc): State<Arc<EcosystemService>>,
    Json(req): Json<PathfindingRequest>,
) -> impl IntoResponse {
    match svc.find_path(req).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
