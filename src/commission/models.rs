//! Domain models for the commission management engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enums (mirror DB enums)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "commission_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CommissionType {
    Percentage,
    FixedFiat,
    Tiered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "ledger_direction", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LedgerDirection {
    Credit,
    Debit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "payout_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PayoutStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

// ---------------------------------------------------------------------------
// Tier definition (stored in JSONB)
// ---------------------------------------------------------------------------

/// Single tier in a tiered commission structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommissionTier {
    /// Minimum cumulative volume (stroops, inclusive)
    pub min_volume_stroops: i64,
    /// Maximum cumulative volume (stroops, exclusive); None = unlimited
    pub max_volume_stroops: Option<i64>,
    /// Rate applied for this tier (0.0–1.0)
    pub rate: f64,
}

// ---------------------------------------------------------------------------
// commission_structures row
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommissionStructure {
    pub id: Uuid,
    pub partner_id: Uuid,
    pub name: String,
    pub commission_type: CommissionType,
    pub percentage_rate: Option<sqlx::types::BigDecimal>,
    pub fixed_stroops: Option<i64>,
    pub tiers: Option<serde_json::Value>,
    pub min_volume_stroops: i64,
    pub max_volume_stroops: Option<i64>,
    pub corridor: Option<String>,
    pub is_active: bool,
    pub effective_from: DateTime<Utc>,
    pub effective_to: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// partner_revenue_ledger row
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LedgerEntry {
    pub entry_id: Uuid,
    pub partner_id: Uuid,
    pub transaction_id: Uuid,
    pub commission_structure_id: Option<Uuid>,
    pub amount_stroops: i64,
    pub direction: LedgerDirection,
    pub balance_after_stroops: i64,
    pub gross_fee_stroops: i64,
    pub platform_share_stroops: i64,
    pub tier_index: Option<i16>,
    pub corridor: Option<String>,
    pub narrative: String,
    pub stellar_tx_hash: Option<String>,
    pub payout_record_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// commission_payout_records row
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PayoutRecord {
    pub id: Uuid,
    pub partner_id: Uuid,
    pub payout_address: String,
    pub total_stroops: i64,
    pub entry_count: i32,
    pub status: PayoutStatus,
    pub stellar_tx_hash: Option<String>,
    pub batch_ref: String,
    pub initiated_by: Uuid,
    pub error_message: Option<String>,
    pub attempted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// partner_commission_balances row
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommissionBalance {
    pub partner_id: Uuid,
    pub accrued_stroops: i64,
    pub paid_stroops: i64,
    pub last_entry_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Input structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCommissionStructureInput {
    pub partner_id: Uuid,
    pub name: String,
    pub commission_type: CommissionType,
    pub percentage_rate: Option<f64>,
    pub fixed_stroops: Option<i64>,
    pub tiers: Option<Vec<CommissionTier>>,
    pub min_volume_stroops: Option<i64>,
    pub max_volume_stroops: Option<i64>,
    pub corridor: Option<String>,
    pub effective_from: Option<DateTime<Utc>>,
    pub effective_to: Option<DateTime<Utc>>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManualAdjustmentInput {
    pub partner_id: Uuid,
    pub transaction_id: Uuid,
    pub amount_stroops: i64,
    pub direction: LedgerDirection,
    pub gross_fee_stroops: i64,
    pub platform_share_stroops: i64,
    pub narrative: String,
    pub initiated_by: Uuid,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RevenueStatement {
    pub partner_id: Uuid,
    pub accrued_stroops: i64,
    pub paid_stroops: i64,
    pub unpaid_stroops: i64,
    pub entries: Vec<LedgerEntry>,
    pub payouts: Vec<PayoutRecord>,
    pub generated_at: DateTime<Utc>,
}
