//! #491 Compliance Oracle & Identity Verification Bridge — data models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Enums ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "oracle_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum OracleStatus {
    Active,
    Degraded,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "did_method", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DidMethod {
    DidWeb,
    DidKey,
    DidEthr,
    DidStellar,
    DidIon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "attestation_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AttestationStatus {
    Valid,
    Expired,
    Revoked,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "query_outcome", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum QueryOutcome {
    Cleared,
    Blocked,
    AmberReview,
    Error,
    CacheHit,
    TimedOut,
}

// ── DB rows ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ComplianceOracle {
    pub oracle_id: Uuid,
    pub name: String,
    pub endpoint_url: String,
    pub status: OracleStatus,
    pub public_key: String,
    pub price_per_query_usd_cents: i32,
    pub sla_ms: i32,
    pub priority: i32,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IdentityAttestation {
    pub attestation_id: Uuid,
    pub cross_chain_address: String,
    pub did_identifier: String,
    pub did_method: DidMethod,
    pub oracle_id: Uuid,
    pub proof_hash: String,
    pub zk_proof_ref: Option<String>,
    pub issuer_signature: String,
    pub status: AttestationStatus,
    pub is_sanctions_clear: bool,
    pub is_kyc_verified: bool,
    pub is_aml_clear: bool,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ComplianceQueryLog {
    pub query_id: Uuid,
    pub originating_tx_id: Uuid,
    pub cross_chain_address: String,
    pub oracle_id: Option<Uuid>,
    pub outcome: QueryOutcome,
    pub latency_ms: Option<i32>,
    pub cache_hit: bool,
    pub signature_envelope: Option<String>,
    pub error_detail: Option<String>,
    pub queried_at: DateTime<Utc>,
}

// ── In-memory types ───────────────────────────────────────────────────────────

/// Parsed W3C DID document.
#[derive(Debug, Clone)]
pub struct ParsedDid {
    pub method: DidMethod,
    pub identifier: String,
    pub controller: String,
}

/// Result of a full compliance verification.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub address: String,
    pub outcome: QueryOutcome,
    pub attestation_id: Option<Uuid>,
    pub from_cache: bool,
    pub latency_ms: u64,
}
