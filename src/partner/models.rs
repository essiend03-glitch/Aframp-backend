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

// ── Issue #466: Partner Integration Framework Data Model ──────────────────────

/// Core partner entity — business classification, onboarding lifecycle, compliance tier.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartnerEntity {
    pub id: Uuid,
    pub legal_name: String,
    pub trading_name: Option<String>,
    /// "commercial_bank" | "mobile_money_operator" | "fintech" | "microfinance" | "payment_aggregator" | "other"
    pub organisation_type: String,
    pub registration_number: String,
    pub jurisdiction: String, // ISO 3166-1 alpha-2
    /// "sandbox" | "testing" | "verified" | "production"
    pub onboarding_state: String,
    /// "standard" | "enhanced" | "premium"
    pub compliance_tier: String,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Extended business and contact details for a partner.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartnerProfile {
    pub partner_id: Uuid,
    pub primary_contact_name: String,
    pub primary_contact_email: String,
    pub primary_contact_phone: Option<String>,
    pub technical_contact_email: Option<String>,
    pub compliance_contact_email: Option<String>,
    pub website_url: Option<String>,
    pub support_url: Option<String>,
    pub logo_url: Option<String>,
    pub regulatory_licence_ref: Option<String>,
    pub regulated_by: Option<String>,
    pub notes: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Secure API credentials: salted hash, asymmetric public key, IP whitelist, webhook config.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartnerApiCredential {
    pub id: Uuid,
    pub partner_id: Uuid,
    pub api_key_hash: String,
    pub api_key_salt: String,
    pub api_key_prefix: String,
    pub public_signing_key: Option<String>,
    /// "Ed25519" | "RSA-2048" | "ES256"
    pub signing_algorithm: Option<String>,
    pub ip_whitelist: Vec<String>,
    pub webhook_url: Option<String>,
    pub webhook_secret_hash: Option<String>,
    /// "sandbox" | "production"
    pub environment: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePartnerRequest {
    pub legal_name: String,
    pub trading_name: Option<String>,
    pub organisation_type: String,
    pub registration_number: String,
    pub jurisdiction: String,
    pub compliance_tier: Option<String>,
    // Profile fields (created together)
    pub primary_contact_name: String,
    pub primary_contact_email: String,
    pub primary_contact_phone: Option<String>,
    pub technical_contact_email: Option<String>,
    pub compliance_contact_email: Option<String>,
    pub regulatory_licence_ref: Option<String>,
    pub regulated_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueApiCredentialRequest {
    pub environment: String,
    pub public_signing_key: Option<String>,
    pub signing_algorithm: Option<String>,
    pub ip_whitelist: Option<Vec<String>>,
    pub webhook_url: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Returned once at issuance — raw key is never stored.
#[derive(Debug, Clone, Serialize)]
pub struct IssuedApiCredential {
    pub credential_id: Uuid,
    pub api_key: String,       // raw key — show once
    pub api_key_prefix: String,
    pub environment: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateOnboardingStateRequest {
    pub onboarding_state: String,
}
