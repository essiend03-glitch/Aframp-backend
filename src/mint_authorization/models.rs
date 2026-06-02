//! Domain models for the Mint Authorization Framework (#213).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Status enum
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "mint_auth_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MintAuthStatus {
    PendingSignatures,
    ThresholdMet,
    Submitted,
    Confirmed,
    Failed,
    Expired,
    Cancelled,
}

impl MintAuthStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Confirmed | Self::Failed | Self::Expired | Self::Cancelled
        )
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::PendingSignatures | Self::ThresholdMet)
    }
}

impl std::fmt::Display for MintAuthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::PendingSignatures => "pending_signatures",
            Self::ThresholdMet => "threshold_met",
            Self::Submitted => "submitted",
            Self::Confirmed => "confirmed",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        };
        f.write_str(s)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Core domain structs
// ─────────────────────────────────────────────────────────────────────────────

/// A mint authorization request awaiting M-of-N signatures.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MintAuthRequest {
    pub id: Uuid,
    pub amount_cngn: BigDecimal,
    pub destination_account: String,
    pub requested_by: Uuid,
    pub requested_by_key: String,
    pub justification: String,
    pub reserve_verification_id: Uuid,
    pub required_signatures: i16,
    pub collected_signatures: i16,
    pub unsigned_xdr: String,
    pub signed_xdr: Option<String>,
    /// SHA-256 hash (hex) of the transaction that all signers must sign.
    pub tx_hash: Option<String>,
    pub stellar_tx_hash: Option<String>,
    pub status: MintAuthStatus,
    pub failure_reason: Option<String>,
    pub cancellation_reason: Option<String>,
    pub cancelled_by: Option<Uuid>,
    pub retry_count: i16,
    pub expires_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single signer's Ed25519 signature on a mint authorization request.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MintAuthSignature {
    pub id: Uuid,
    pub auth_request_id: Uuid,
    pub signer_id: Uuid,
    pub signer_key: String,
    /// Base64-encoded raw 64-byte Ed25519 signature over `tx_hash`.
    pub signature: String,
    pub signed_at: DateTime<Utc>,
    pub ip_address: Option<ipnetwork::IpNetwork>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Request / Response DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// POST /api/admin/mint/authorizations
#[derive(Debug, Deserialize)]
pub struct CreateMintAuthRequest {
    /// Amount of cNGN to mint (decimal string, e.g. "1000000.0000000").
    pub amount_cngn: String,
    /// Stellar distribution account address (G…).
    pub destination_account: String,
    /// Human-readable justification for the mint.
    pub justification: String,
    /// ID of the reserve verification snapshot to anchor this request.
    pub reserve_verification_id: Uuid,
}

/// POST /api/admin/mint/authorizations/:auth_id/sign
#[derive(Debug, Deserialize)]
pub struct SubmitSignatureRequest {
    /// Base64-encoded raw 64-byte Ed25519 signature over the `tx_hash`.
    pub signature: String,
    /// Signer's Stellar public key (G…) — must match the registered key.
    pub signer_key: String,
}

/// POST /api/admin/mint/authorizations/:auth_id/cancel
#[derive(Debug, Deserialize)]
pub struct CancelMintAuthRequest {
    pub justification: String,
}

/// Full detail response for a mint authorization request.
#[derive(Debug, Serialize)]
pub struct MintAuthDetail {
    pub request: MintAuthRequest,
    pub signatures: Vec<MintAuthSignature>,
    pub signatures_collected: usize,
    pub signatures_required: usize,
}

/// Paginated list response.
#[derive(Debug, Serialize)]
pub struct MintAuthListResponse {
    pub items: Vec<MintAuthRequest>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Query params for listing.
#[derive(Debug, Deserialize)]
pub struct ListMintAuthQuery {
    pub status: Option<MintAuthStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl ListMintAuthQuery {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(20).clamp(1, 100)
    }
    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}
