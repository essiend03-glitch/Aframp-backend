use crate::services::balance::{BalanceService, WalletBalance};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Clone)]
pub struct WalletState {
    pub balance_service: Arc<BalanceService>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceQuery {
    pub address: String,
    #[serde(default)]
    pub refresh: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_address: Option<String>,
}

pub async fn get_balance(
    State(state): State<WalletState>,
    Query(params): Query<BalanceQuery>,
) -> Response {
    info!(
        "Balance request for address: {}, refresh: {}",
        params.address, params.refresh
    );

    match state
        .balance_service
        .get_balance(&params.address, params.refresh)
        .await
    {
        Ok(balance) => (StatusCode::OK, Json(balance)).into_response(),
        Err(e) => handle_error(e, &params.address),
    }
}

fn handle_error(error: crate::chains::stellar::errors::StellarError, address: &str) -> Response {
// REMOVED:     use crate::chains::stellar::errors::StellarError;

    match error {
        StellarError::InvalidAddress { .. } => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ErrorDetail {
                    code: "INVALID_ADDRESS".to_string(),
                    message: "Invalid Stellar wallet address format".to_string(),
                    details: Some(
                        "Stellar addresses must be 56 characters starting with 'G'".to_string(),
                    ),
                    wallet_address: None,
                },
            }),
        )
            .into_response(),

        StellarError::AccountNotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: ErrorDetail {
                    code: "WALLET_NOT_FOUND".to_string(),
                    message: "Wallet address not found on Stellar network".to_string(),
                    details: Some(
                        "This wallet has not been activated. Fund it with at least 1 XLM to activate.".to_string(),
                    ),
                    wallet_address: Some(address.to_string()),
                },
            }),
        )
            .into_response(),

        StellarError::RateLimitError => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                error: ErrorDetail {
                    code: "RATE_LIMIT_ERROR".to_string(),
                    message: "Too many requests, please try again".to_string(),
                    details: Some("Stellar API rate limit reached".to_string()),
                    wallet_address: None,
                },
            }),
        )
            .into_response(),

        StellarError::TimeoutError { .. } | StellarError::NetworkError { .. } => {
            error!("Stellar network error: {}", error);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: ErrorDetail {
                        code: "NETWORK_UNAVAILABLE".to_string(),
                        message: "Stellar network temporarily unavailable".to_string(),
                        details: Some("Please try again in a few moments".to_string()),
                        wallet_address: None,
                    },
                }),
            )
                .into_response()
        }

        _ => {
            error!("Unexpected error fetching balance: {}", error);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: ErrorDetail {
                        code: "INTERNAL_ERROR".to_string(),
                        message: "An unexpected error occurred".to_string(),
                        details: None,
                        wallet_address: None,
                    },
                }),
            )
                .into_response()
        }
    }
}
