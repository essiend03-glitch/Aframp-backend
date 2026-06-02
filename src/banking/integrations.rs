//! Bank Integration Models - Corporate banking partners and virtual accounts

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Bank Integration - Corporate banking partner configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankIntegration {
    pub id: Uuid,
    pub partner_name: String,
    pub partner_code: String,
    pub api_base_url: String,
    pub api_key_secret_ref: String,
    pub webhook_secret_ref: String,
    pub status: BankIntegrationStatus,
    pub settlement_pool_account: Option<String>,
    pub settlement_bank_code: Option<String>,
    pub settlement_account_name: Option<String>,
    pub settlement_account_number: Option<String>,
    pub priority_weight: i32,
    pub rate_limit_rpm: i32,
    pub timeout_seconds: i32,
    pub health_check_url: Option<String>,
    pub last_health_check: Option<DateTime<Utc>>,
    pub last_health_status: Option<String>,
    pub webhook_backlog_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum BankIntegrationStatus {
    Active,
    Inactive,
    Suspended,
}

impl BankIntegrationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BankIntegrationStatus::Active => "active",
            BankIntegrationStatus::Inactive => "inactive",
            BankIntegrationStatus::Suspended => "suspended",
        }
    }
}

impl std::fmt::Display for BankIntegrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Virtual Account - Dedicated inbound payment accounts
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VirtualAccount {
    pub id: Uuid,
    pub user_id: Uuid,
    pub bank_integration_id: Option<Uuid>,
    pub virtual_account_number: String,
    pub virtual_account_name: String,
    pub bank_code: String,
    pub bank_name: String,
    pub assignment_state: VirtualAccountState,
    pub settlement_tracking_code: Option<String>,
    pub expected_amount: Option<Decimal>,
    pub expected_currency: String,
    pub settlement_reference: Option<String>,
    pub settled_amount: Decimal,
    pub settled_at: Option<DateTime<Utc>>,
    pub last_transaction_at: Option<DateTime<Utc>>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum VirtualAccountState {
    Pending,
    Active,
    Suspended,
    Closed,
}

impl VirtualAccountState {
    pub fn as_str(&self) -> &'static str {
        match self {
            VirtualAccountState::Pending => "pending",
            VirtualAccountState::Active => "active",
            VirtualAccountState::Suspended => "suspended",
            VirtualAccountState::Closed => "closed",
        }
    }
}

// ============================================================================
// Fiat Settlement - Fiat deposits and cNGN minting
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FiatSettlement {
    pub id: Uuid,
    pub virtual_account_id: Uuid,
    pub user_id: Uuid,
    pub bank_integration_id: Option<Uuid>,
    pub bank_transaction_id: String,
    pub bank_reference: Option<String>,
    pub amount: Decimal,
    pub currency: String,
    pub cngn_amount: Option<Decimal>,
    pub cngn_minted: bool,
    pub wallet_address: Option<String>,
    pub settlement_status: SettlementStatus,
    pub settlement_error: Option<String>,
    pub webhook_event_id: Option<Uuid>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub minted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum SettlementStatus {
    Pending,
    Confirmed,
    Minting,
    Completed,
    Failed,
}

impl SettlementStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SettlementStatus::Pending => "pending",
            SettlementStatus::Confirmed => "confirmed",
            SettlementStatus::Minting => "minting",
            SettlementStatus::Completed => "completed",
            SettlementStatus::Failed => "failed",
        }
    }
}

// ============================================================================
// Bank Webhook - Enhanced webhook tracking
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankWebhook {
    pub id: Uuid,
    pub bank_integration_id: Option<Uuid>,
    pub event_type: String,
    pub provider_event_id: String,
    pub payload: serde_json::Value,
    pub signature_valid: Option<bool>,
    pub signature_algorithm: Option<String>,
    pub idempotency_key: Option<String>,
    pub processing_status: WebhookProcessingStatus,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub processed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum WebhookProcessingStatus {
    Received,
    Processing,
    Processed,
    Failed,
    Duplicate,
}

impl WebhookProcessingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WebhookProcessingStatus::Received => "received",
            WebhookProcessingStatus::Processing => "processing",
            WebhookProcessingStatus::Processed => "processed",
            WebhookProcessingStatus::Failed => "failed",
            WebhookProcessingStatus::Duplicate => "duplicate",
        }
    }
}

// ============================================================================
// Bank API Metrics
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankApiMetric {
    pub id: Uuid,
    pub bank_integration_id: Uuid,
    pub api_endpoint: String,
    pub method: String,
    pub latency_ms: i64,
    pub status_code: Option<i32>,
    pub error_code: Option<String>,
    pub request_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Reconciliation Job
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankReconciliationJob {
    pub id: Uuid,
    pub bank_integration_id: Uuid,
    pub trigger_type: ReconciliationTriggerType,
    pub triggered_by: Option<Uuid>,
    pub status: ReconciliationJobStatus,
    pub records_checked: i32,
    pub discrepancies_found: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum ReconciliationTriggerType {
    Scheduled,
    Manual,
    Automatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum ReconciliationJobStatus {
    Running,
    Completed,
    Failed,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateVirtualAccountRequest {
    pub user_id: Uuid,
    pub bank_integration_id: Option<Uuid>,
    pub expected_amount: Option<Decimal>,
    pub expected_currency: Option<String>,
    pub is_primary: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyAccountRequest {
    pub account_number: String,
    pub bank_code: String,
}

#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    pub event_type: String,
    pub event_id: String,
    pub data: serde_json::Value,
    pub signature: Option<String>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ReconcileRequest {
    pub bank_integration_id: Uuid,
    pub date_from: Option<chrono::NaiveDate>,
    pub date_to: Option<chrono::NaiveDate>,
}

#[derive(Debug, Serialize)]
pub struct BankPartnerStatusResponse {
    pub partner_code: String,
    pub partner_name: String,
    pub status: String,
    pub connection_health: String,
    pub settlement_pool_balance: Option<Decimal>,
    pub webhook_backlog_count: i32,
    pub last_health_check: Option<DateTime<Utc>>,
    pub api_latency_ms: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct FiatSettlementResponse {
    pub settlement_id: Uuid,
    pub user_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub cngn_amount: Option<Decimal>,
    pub cngn_minted: bool,
    pub status: String,
    pub wallet_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}