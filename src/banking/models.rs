//! Banking Integration — Domain Types (Issue #407)

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

// ── Linked Bank Account ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LinkedBankAccount {
    pub id: Uuid,
    pub user_id: Uuid,
    /// Tokenized reference — never the raw account number
    pub account_token: String,
    /// Masked display value e.g. "****1234"
    pub account_mask: String,
    pub account_name: String,
    pub bank_code: String,
    pub bank_name: String,
    pub currency: String,
    pub status: String,
    /// SHA-256 hash of BVN/NIN — never plaintext
    pub identity_hash: Option<String>,
    pub verified_by: String,
    pub verified_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LinkAccountRequest {
    pub account_number: String,
    pub bank_code: String,
    pub account_name: String,
    /// BVN or NIN for identity verification
    pub identity_number: String,
}

// ── Bank Mandate ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankMandate {
    pub id: Uuid,
    pub linked_account_id: Uuid,
    pub user_id: Uuid,
    pub mandate_type: String,
    pub status: String,
    /// Maximum single-transaction amount in minor units (kobo)
    pub max_amount: i64,
    pub provider_reference: String,
    pub provider: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMandateRequest {
    pub linked_account_id: Uuid,
    pub mandate_type: String,
    pub max_amount: i64,
}

// ── Bank Transfer Log ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankTransferLog {
    pub id: Uuid,
    pub idempotency_key: String,
    pub mandate_id: Option<Uuid>,
    pub linked_account_id: Uuid,
    pub direction: String,
    pub amount: i64,
    pub currency: String,
    pub status: String,
    pub provider: String,
    pub provider_reference: Option<String>,
    pub provider_response: Option<serde_json::Value>,
    pub failure_reason: Option<String>,
    pub settled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct InitiateTransferRequest {
    pub linked_account_id: Uuid,
    pub mandate_id: Option<Uuid>,
    pub direction: TransferDirection,
    pub amount: i64,
    pub currency: String,
    /// Caller-supplied idempotency key
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferDirection {
    Debit,
    Credit,
}

impl std::fmt::Display for TransferDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferDirection::Debit => write!(f, "debit"),
            TransferDirection::Credit => write!(f, "credit"),
        }
    }
}

// ── Reconciliation ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankReconciliationRun {
    pub id: Uuid,
    pub run_date: NaiveDate,
    pub bank_code: String,
    pub status: String,
    pub aframp_total: BigDecimal,
    pub bank_total: BigDecimal,
    pub discrepancy: BigDecimal,
    pub flagged_count: i32,
    pub metadata: Option<serde_json::Value>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── Webhook Event ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankWebhookEvent {
    pub id: Uuid,
    pub provider: String,
    pub event_type: String,
    pub provider_event_id: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub linked_account_id: Option<Uuid>,
    pub transfer_log_id: Option<Uuid>,
    pub error_message: Option<String>,
    pub processed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
