use crate::cbdc::models::*;
use crate::cbdc::repository::CbdcRepository;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ListSwapsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListGatewaysQuery {
    pub active_only: Option<bool>,
}

#[instrument(skip(state))]
pub async fn register_gateway(
    State(state): State<Arc<CbdcHandlerState>>,
    Json(req): Json<RegisterGatewayRequest>,
) -> Result<Json<CbdcGateway>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    match state.repo.register_gateway(&req).await {
        Ok(gateway) => {
            info!(gateway_id = %gateway.id, name = %gateway.name, "CBDC gateway registered");
            Ok(Json(gateway))
        }
        Err(e) => {
            error!(error = %e, "Failed to register CBDC gateway");
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to register gateway: {}", e)})),
            ))
        }
    }
}

#[instrument(skip(state))]
pub async fn list_gateways(
    State(state): State<Arc<CbdcHandlerState>>,
    Query(_query): Query<ListGatewaysQuery>,
) -> Result<Json<Vec<CbdcGateway>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    match state.repo.list_gateways().await {
        Ok(gateways) => Ok(Json(gateways)),
        Err(e) => {
            error!(error = %e, "Failed to list CBDC gateways");
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to list gateways: {}", e)})),
            ))
        }
    }
}

#[instrument(skip(state))]
pub async fn get_gateway(
    State(state): State<Arc<CbdcHandlerState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let gateway = state
        .repo
        .get_gateway(id)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Gateway not found"})),
            )
        })?;

    Ok(Json(serde_json::json!(gateway)))
}

#[instrument(skip(state))]
pub async fn initiate_swap(
    State(state): State<Arc<CbdcHandlerState>>,
    Json(req): Json<InitiateSwapRequest>,
) -> Result<Json<CbdcSwapRecord>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    // Check idempotency
    if let Ok(Some(existing)) = state.repo.get_swap_by_idempotency(&req.idempotency_key).await {
        return Ok(Json(existing));
    }

    match state.repo.create_swap_record(&req).await {
        Ok(record) => {
            info!(
                swap_id = %record.id,
                swap_type = %record.swap_type,
                "CBDC swap initiated"
            );
            Ok(Json(record))
        }
        Err(e) => {
            error!(error = %e, "Failed to create CBDC swap record");
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to initiate swap: {}", e)})),
            ))
        }
    }
}

#[instrument(skip(state))]
pub async fn get_swap_status(
    State(state): State<Arc<CbdcHandlerState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<SwapStatusResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let record = state
        .repo
        .get_swap_record(id)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Swap record not found"})),
            )
        })?;

    Ok(Json(SwapStatusResponse {
        id: record.id,
        swap_type: record.swap_type,
        status: record.status,
        two_phase_state: record.two_phase_state,
        stellar_transaction_hash: record.stellar_transaction_hash,
        cbdc_transaction_id: record.cbdc_transaction_id,
        cbdc_block_id: record.cbdc_block_id,
        cbdc_confirmations: record.cbdc_confirmations,
        aml_screening_result: record.aml_screening_result,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }))
}

#[instrument(skip(state))]
pub async fn list_swaps(
    State(state): State<Arc<CbdcHandlerState>>,
    Query(query): Query<ListSwapsQuery>,
) -> Result<Json<Vec<CbdcSwapRecord>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    match state
        .repo
        .list_swaps(limit, offset, query.status.as_deref())
        .await
    {
        Ok(records) => Ok(Json(records)),
        Err(e) => {
            error!(error = %e, "Failed to list CBDC swaps");
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to list swaps: {}", e)})),
            ))
        }
    }
}

#[instrument(skip(state))]
pub async fn get_swap_signatories(
    State(state): State<Arc<CbdcHandlerState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<CryptographicSignatory>>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    match state.repo.get_signatories_for_swap(id).await {
        Ok(signatories) => Ok(Json(signatories)),
        Err(e) => {
            error!(error = %e, "Failed to fetch signatories");
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            ))
        }
    }
}

pub struct CbdcHandlerState {
    pub repo: Arc<CbdcRepository>,
}

impl CbdcHandlerState {
    pub fn new(repo: Arc<CbdcRepository>) -> Self {
        Self { repo }
    }
}
