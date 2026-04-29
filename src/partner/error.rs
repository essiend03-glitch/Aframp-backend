use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum PartnerError {
    #[error("Partner not found")]
    NotFound,
    #[error("Partner already registered with this organisation")]
    AlreadyExists,
    #[error("Credential not found")]
    CredentialNotFound,
    #[error("Credential revoked")]
    CredentialRevoked,
    #[error("Credential expired")]
    CredentialExpired,
    #[error("Invalid credential type: {0}")]
    InvalidCredentialType(String),
    #[error("Invalid partner type: {0}")]
    InvalidPartnerType(String),
    #[error("Partner suspended")]
    Suspended,
    #[error("IP not whitelisted")]
    IpNotWhitelisted,
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("API version deprecated: {0}")]
    VersionDeprecated(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Crypto error: {0}")]
    Crypto(String),
}

impl IntoResponse for PartnerError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::NotFound | Self::CredentialNotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::AlreadyExists => (StatusCode::CONFLICT, "already_exists"),
            Self::CredentialRevoked => (StatusCode::UNAUTHORIZED, "credential_revoked"),
            Self::CredentialExpired => (StatusCode::UNAUTHORIZED, "credential_expired"),
            Self::InvalidCredentialType(_) | Self::InvalidPartnerType(_) => {
                (StatusCode::BAD_REQUEST, "invalid_request")
            }
            Self::Suspended => (StatusCode::FORBIDDEN, "partner_suspended"),
            Self::IpNotWhitelisted => (StatusCode::FORBIDDEN, "ip_not_whitelisted"),
            Self::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "rate_limit_exceeded"),
            Self::VersionDeprecated(_) => (StatusCode::GONE, "version_deprecated"),
            Self::ValidationFailed(_) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed"),
            Self::Database(_) | Self::Crypto(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        };
        (
            status,
            Json(json!({"error": {"code": code, "message": self.to_string()}})),
        )
            .into_response()
    }
}
