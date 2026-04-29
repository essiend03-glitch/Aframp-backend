//! Data models for wallet provisioning state machine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Provisioning states
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningState {
    KeypairGenerated,
    Registered,
    PendingFunding,
    Funded,
    TrustlinePending,
    TrustlineActive,
    Ready,
    Stalled,
    Failed,
}

impl ProvisioningState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::KeypairGenerated => "keypair_generated",
            Self::Registered => "registered",
            Self::PendingFunding => "pending_funding",
            Self::Funded => "funded",
            Self::TrustlinePending => "trustline_pending",
            Self::TrustlineActive => "trustline_active",
            Self::Ready => "ready",
            Self::Stalled => "stalled",
            Self::Failed => "failed",
        }
    }

    pub fn next_step_instructions(&self) -> &'static str {
        match self {
            Self::KeypairGenerated => "Generate a BIP-39 mnemonic and derive your Stellar keypair. Write down your 24-word seed phrase and store it securely offline.",
            Self::Registered => "Your wallet is registered. Fund your Stellar account with the minimum required XLM balance.",
            Self::PendingFunding => "Waiting for your Stellar account to be funded. Send XLM to your wallet address or use platform-sponsored creation.",
            Self::Funded => "Account funded. Initiate the cNGN trustline by signing the provided transaction envelope.",
            Self::TrustlinePending => "Trustline submitted. Waiting for issuer authorization of your cNGN trustline.",
            Self::TrustlineActive => "Trustline active. Verifying all readiness criteria.",
            Self::Ready => "Your wallet is fully provisioned and ready to send and receive cNGN.",
            Self::Stalled => "Provisioning has stalled. Please check the status endpoint for troubleshooting guidance.",
            Self::Failed => "Provisioning failed. See the failure reason and retry guidance in the status response.",
        }
    }
}

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WalletProvisioning {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub state: String,
    pub is_sponsored: bool,
    pub sponsorship_tx_hash: Option<String>,
    pub sponsorship_xlm_amount: Option<sqlx::types::BigDecimal>,
    pub funding_method: Option<String>,
    pub funding_detected_at: Option<DateTime<Utc>>,
    pub funding_tx_hash: Option<String>,
    pub trustline_envelope: Option<String>,
    pub trustline_submitted_at: Option<DateTime<Utc>>,
    pub trustline_tx_hash: Option<String>,
    pub trustline_authorized_at: Option<DateTime<Utc>>,
    pub became_ready_at: Option<DateTime<Utc>>,
    pub last_failure_reason: Option<String>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub step_started_at: DateTime<Utc>,
    pub step_timeout_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PlatformFundingAccount {
    pub id: Uuid,
    pub stellar_address: String,
    pub current_xlm_balance: sqlx::types::BigDecimal,
    pub total_accounts_sponsored: i32,
    pub total_xlm_spent: sqlx::types::BigDecimal,
    pub min_balance_alert_threshold: sqlx::types::BigDecimal,
    pub eligibility_criteria: serde_json::Value,
    pub is_active: bool,
    pub last_balance_check_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WalletReadinessCheck {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub stellar_account_exists: bool,
    pub min_xlm_balance_met: bool,
    pub trustline_active: bool,
    pub trustline_authorized: bool,
    pub wallet_registered: bool,
    pub all_criteria_met: bool,
    pub checked_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Funding requirements response.
#[derive(Debug, Serialize)]
pub struct FundingRequirements {
    pub wallet_id: Uuid,
    pub base_reserve_xlm: f64,
    pub trustline_reserve_xlm: f64,
    pub fee_buffer_xlm: f64,
    pub total_required_xlm: f64,
    pub sponsorship_available: bool,
}

/// Provisioning status response (resumable).
#[derive(Debug, Serialize)]
pub struct ProvisioningStatus {
    pub wallet_id: Uuid,
    pub state: String,
    pub next_step: String,
    pub instructions: String,
    pub is_sponsored: bool,
    pub funding_method: Option<String>,
    pub last_failure_reason: Option<String>,
    pub retry_count: i32,
    pub step_timeout_at: Option<DateTime<Utc>>,
    pub became_ready_at: Option<DateTime<Utc>>,
}

/// Trustline initiation response — unsigned XDR envelope for client signing.
#[derive(Debug, Serialize)]
pub struct TrustlineInitiateResponse {
    pub wallet_id: Uuid,
    pub unsigned_envelope_xdr: String,
    pub asset_code: String,
    pub issuer: String,
    pub instructions: String,
}

/// Trustline submission request — signed XDR from client.
#[derive(Debug, Deserialize)]
pub struct TrustlineSubmitRequest {
    pub signed_envelope_xdr: String,
}

/// Wallet readiness response.
#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub wallet_id: Uuid,
    pub is_ready: bool,
    pub criteria: ReadinessCriteria,
    pub pending_steps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReadinessCriteria {
    pub stellar_account_exists: bool,
    pub min_xlm_balance_met: bool,
    pub trustline_active: bool,
    pub trustline_authorized: bool,
    pub wallet_registered: bool,
}

/// Admin funding account status.
#[derive(Debug, Serialize)]
pub struct FundingAccountStatus {
    pub stellar_address: String,
    pub current_xlm_balance: String,
    pub total_accounts_sponsored: i32,
    pub total_xlm_spent: String,
    pub estimated_remaining_capacity: i64,
    pub min_balance_alert_threshold: String,
    pub is_below_threshold: bool,
}

/// Admin replenishment request.
#[derive(Debug, Deserialize)]
pub struct ReplenishmentRequest {
    pub requested_xlm_amount: f64,
    pub notes: Option<String>,
}

/// Sponsorship eligibility check result.
#[derive(Debug, Serialize)]
pub struct SponsorshipEligibility {
    pub eligible: bool,
    pub reason: Option<String>,
}
