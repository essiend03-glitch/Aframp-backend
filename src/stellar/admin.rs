/// Admin endpoints for Stellar submission channel management
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sqlx::PgPool;

use crate::stellar::submission::StellarSubmissionEngine;
use crate::stellar::error::SubmissionResult;
use crate::admin::auth::AdminAuthLayer;

/// Admin state for stellar routes
pub struct StellarAdminState {
    pub pool: PgPool,
    pub submission_engine: std::sync::Arc<StellarSubmissionEngine>,
}

/// Channel status response
#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelStatusResponse {
    pub channel_id: String,
    pub index: i32,
    pub account_id: String,
    pub balance_xlm: Decimal,
    pub current_sequence: i64,
    pub reserved_sequence: i64,
    pub in_flight_transactions: i64,
    pub total_submitted: u64,
    pub total_successful: u64,
    pub total_failed: u64,
    pub consecutive_failures: u32,
    pub is_circuit_broken: bool,
    pub status: String,
}

/// Channel top-up request
#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelTopUpRequest {
    pub channel_index: i32,
    pub amount_xlm: Decimal,
    pub description: Option<String>,
}

/// Admin response
#[derive(Debug, Serialize, Deserialize)]
pub struct AdminResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// Get all submission channels status
async fn get_channels(
    State(state): State<StellarAdminState>,
) -> Result<Json<AdminResponse<Vec<ChannelStatusResponse>>>, AdminError> {
    let stats = state.submission_engine.get_pool_stats().await?;

    let channels: Vec<ChannelStatusResponse> = stats
        .iter()
        .map(|stat| {
            let balance = stat["balance_xlm"]
                .as_f64()
                .unwrap_or(0.0);
            let status = if stat["is_circuit_broken"].as_bool().unwrap_or(false) {
                "circuit_broken".to_string()
            } else if stat["in_flight"].as_i64().unwrap_or(0) > 900 {
                "exhausted".to_string()
            } else {
                "healthy".to_string()
            };

            ChannelStatusResponse {
                channel_id: stat["channel_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                index: stat["index"].as_i64().unwrap_or(0) as i32,
                account_id: stat["account_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                balance_xlm: sqlx::types::Decimal::from_f64_retain(balance).unwrap_or_default(),
                current_sequence: stat["current_sequence"].as_i64().unwrap_or(0),
                reserved_sequence: stat["reserved_sequence"].as_i64().unwrap_or(0),
                in_flight_transactions: stat["in_flight"].as_i64().unwrap_or(0),
                total_submitted: stat["total_submitted"].as_u64().unwrap_or(0),
                total_successful: stat["total_successful"].as_u64().unwrap_or(0),
                total_failed: stat["total_failed"].as_u64().unwrap_or(0),
                consecutive_failures: stat["consecutive_failures"].as_u64().unwrap_or(0) as u32,
                is_circuit_broken: stat["is_circuit_broken"].as_bool().unwrap_or(false),
                status,
            }
        })
        .collect();

    Ok(Json(AdminResponse {
        success: true,
        data: Some(channels),
        error: None,
    }))
}

/// Queue a top-up for a channel account
async fn queue_channel_topup(
    State(state): State<StellarAdminState>,
    Path(channel_index): Path<i32>,
    Json(payload): Json<ChannelTopUpRequest>,
) -> Result<Json<AdminResponse<TopUpQueueResponse>>, AdminError> {
    // Validate channel exists
    let _channel = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM stellar_submission_channels WHERE channel_index = $1",
    )
    .bind(channel_index)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| {
        AdminError::NotFound(format!("Channel {} not found", channel_index))
    })?;

    // Queue the top-up operation
    let operation_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO stellar_channel_topup_queue (
            id, channel_index, amount_xlm, description,
            status, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 'pending', NOW(), NOW())
        "#,
    )
    .bind(operation_id)
    .bind(channel_index)
    .bind(payload.amount_xlm)
    .bind(payload.description.unwrap_or_default())
    .execute(&state.pool)
    .await?;

    Ok(Json(AdminResponse {
        success: true,
        data: Some(TopUpQueueResponse {
            operation_id: operation_id.to_string(),
            channel_index,
            amount_xlm: payload.amount_xlm,
            status: "queued".to_string(),
        }),
        error: None,
    }))
}

#[derive(Debug, Serialize)]
pub struct TopUpQueueResponse {
    pub operation_id: String,
    pub channel_index: i32,
    pub amount_xlm: sqlx::types::Decimal,
    pub status: String,
}

/// Admin error type
#[derive(Debug)]
pub enum AdminError {
    NotFound(String),
    BadRequest(String),
    InternalError(String),
    Unauthorized,
}

impl IntoResponse for AdminError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AdminError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AdminError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AdminError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AdminError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "Unauthorized".to_string())
            }
        };

        (
            status,
            Json(AdminResponse::<()> {
                success: false,
                data: None,
                error: Some(message),
            }),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for AdminError {
    fn from(err: sqlx::Error) -> Self {
        AdminError::InternalError(err.to_string())
    }
}

impl<T> From<crate::stellar::error::SubmissionError> for AdminError {
    fn from(err: crate::stellar::error::SubmissionError) -> Self {
        AdminError::InternalError(err.to_string())
    }
}

/// Create admin routes for Stellar management
pub fn stellar_admin_routes() -> Router<StellarAdminState> {
    Router::new()
        .route(
            "/channels",
            get(get_channels),
        )
        .route(
            "/channels/:index/top-up",
            post(queue_channel_topup),
        )
}

use sqlx::types::Decimal;
