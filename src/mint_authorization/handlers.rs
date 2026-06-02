//! HTTP handlers for the Mint Authorization Framework.

use crate::error::{AppError, AppErrorKind, DomainError};
use crate::mint_authorization::{
    error::MintAuthError,
    models::{
        CancelMintAuthRequest, CreateMintAuthRequest, ListMintAuthQuery, SubmitSignatureRequest,
    },
    service::MintAuthService,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

pub type MintAuthState = Arc<MintAuthService>;

fn map_err(e: MintAuthError) -> AppError {
    use MintAuthError::*;
    match e {
        NotFound(id) => AppError::new(AppErrorKind::Domain(DomainError::TransactionNotFound {
            transaction_id: id.to_string(),
        })),
        TerminalState(_, _) | NotReadyForSubmission(_) | DuplicateSignature(_, _) => {
            AppError::new(AppErrorKind::Validation(
                crate::error::ValidationError::InvalidInput {
                    field: "status".into(),
                    message: e.to_string(),
                },
            ))
        }
        UnauthorizedSigner(_) | InvalidSignature(_, _) | TransactionHashMismatch => {
            AppError::new(AppErrorKind::Unauthorized)
        }
        _ => AppError::new(AppErrorKind::Infrastructure(
            crate::error::InfrastructureError::Database {
                message: e.to_string(),
                is_retryable: false,
            },
        )),
    }
}

/// POST /api/admin/mint/authorizations
pub async fn create_authorization(
    State(svc): State<MintAuthState>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, AppError> {
    let (requester_id, requester_key) = extract_requester(&req)?;
    let Json(body): Json<CreateMintAuthRequest> = Json::from_request(req, &())
        .await
        .map_err(|_| AppError::new(AppErrorKind::Validation(
            crate::error::ValidationError::MissingField { field: "body".into() }
        )))?;

    let result = svc.create(body, requester_id, &requester_key).await.map_err(map_err)?;
    Ok((StatusCode::CREATED, Json(result)))
}

/// GET /api/admin/mint/authorizations
pub async fn list_authorizations(
    State(svc): State<MintAuthState>,
    Query(query): Query<ListMintAuthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let result = svc.list(query).await.map_err(map_err)?;
    Ok(Json(result))
}

/// GET /api/admin/mint/authorizations/:auth_id
pub async fn get_authorization(
    State(svc): State<MintAuthState>,
    Path(auth_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let result = svc.get(auth_id).await.map_err(map_err)?;
    Ok(Json(result))
}

/// POST /api/admin/mint/authorizations/:auth_id/sign
pub async fn sign_authorization(
    State(svc): State<MintAuthState>,
    Path(auth_id): Path<Uuid>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, AppError> {
    let ip = extract_ip(&req);
    let Json(body): Json<SubmitSignatureRequest> = Json::from_request(req, &())
        .await
        .map_err(|_| AppError::new(AppErrorKind::Validation(
            crate::error::ValidationError::MissingField { field: "body".into() }
        )))?;

    let result = svc.submit_signature(auth_id, body, ip).await.map_err(map_err)?;
    Ok(Json(result))
}

/// POST /api/admin/mint/authorizations/:auth_id/cancel
pub async fn cancel_authorization(
    State(svc): State<MintAuthState>,
    Path(auth_id): Path<Uuid>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, AppError> {
    let (cancelled_by, _) = extract_requester(&req)?;
    let Json(body): Json<CancelMintAuthRequest> = Json::from_request(req, &())
        .await
        .map_err(|_| AppError::new(AppErrorKind::Validation(
            crate::error::ValidationError::MissingField { field: "body".into() }
        )))?;

    let result = svc.cancel(auth_id, cancelled_by, body).await.map_err(map_err)?;
    Ok(Json(result))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn extract_requester(req: &axum::extract::Request) -> Result<(Uuid, String), AppError> {
    // Try OAuth claims first, then JWT claims
    if let Some(claims) = req.extensions().get::<crate::auth::OAuthTokenClaims>() {
        let id = Uuid::parse_str(&claims.sub).unwrap_or_else(|_| Uuid::nil());
        let key = claims
            .extra
            .get("stellar_public_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        return Ok((id, key));
    }
    if let Some(claims) = req.extensions().get::<crate::auth::jwt::TokenClaims>() {
        let id = Uuid::parse_str(&claims.sub).unwrap_or_else(|_| Uuid::nil());
        return Ok((id, String::new()));
    }
    Err(AppError::new(AppErrorKind::Unauthorized))
}

fn extract_ip(req: &axum::extract::Request) -> Option<std::net::IpAddr> {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
}
