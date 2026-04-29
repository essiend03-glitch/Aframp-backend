use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

// ── Partner entity ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Partner {
    pub id: Uuid,
    pub name: String,
    pub organisation: String,
    pub partner_type: String, // "bank" | "fintech" | "liquidity_provider"
    pub status: String,       // "sandbox" | "active" | "suspended" | "deprecated"
    pub contact_email: String,
    pub ip_whitelist: Vec<String>,
    pub rate_limit_per_minute: i32,
    pub api_version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Credentials ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartnerCredential {
    pub id: Uuid,
    pub partner_id: Uuid,
    pub credential_type: String, // "oauth2_client" | "mtls_cert" | "api_key"
    pub client_id: Option<String>,
    pub client_secret_hash: Option<String>,
    pub certificate_fingerprint: Option<String>,
    pub api_key_hash: Option<String>,
    pub api_key_prefix: Option<String>,
    pub scopes: Vec<String>,
    pub environment: String, // "sandbox" | "production"
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── API version deprecation ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiVersionDeprecation {
    pub id: Uuid,
    pub api_version: String,
    pub deprecated_at: DateTime<Utc>,
    pub sunset_at: DateTime<Utc>,
    pub migration_guide_url: Option<String>,
    pub notified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── Validation test result ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub partner_id: Uuid,
    pub test_name: String,
    pub passed: bool,
    pub detail: String,
    pub tested_at: DateTime<Utc>,
}

// ── Request / response DTOs ───────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RegisterPartnerRequest {
    pub name: String,
    pub organisation: String,
    pub partner_type: String,
    pub contact_email: String,
    pub ip_whitelist: Option<Vec<String>>,
    pub rate_limit_per_minute: Option<i32>,
    pub api_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ProvisionCredentialRequest {
    pub credential_type: String,
    pub environment: String,
    pub scopes: Option<Vec<String>>,
    pub expires_at: Option<DateTime<Utc>>,
    /// PEM-encoded certificate for mTLS provisioning
    pub certificate_pem: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionedCredential {
    pub credential_id: Uuid,
    pub credential_type: String,
    pub environment: String,
    /// Returned once — not stored in plaintext
    pub secret: Option<String>,
    pub client_id: Option<String>,
    pub api_key_prefix: Option<String>,
    pub certificate_fingerprint: Option<String>,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeprecationNotice {
    pub api_version: String,
    pub deprecated_at: DateTime<Utc>,
    pub sunset_at: DateTime<Utc>,
    pub migration_guide_url: Option<String>,
    pub days_until_sunset: i64,
}
