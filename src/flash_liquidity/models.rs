//! #488 Flash Liquidity Provisioning — data models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

// ── Enums ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "facility_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum FacilityStatus {
    Active,
    Suspended,
    Exhausted,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "draw_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DrawStatus {
    Pending,
    CollateralLocked,
    Disbursed,
    Repaid,
    Defaulted,
    RolledBack,
}

// ── DB rows ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CreditFacility {
    pub facility_id: Uuid,
    pub lender_name: String,
    pub lender_api_endpoint: String,
    pub max_drawdown_amount: BigDecimal,
    pub current_utilization: BigDecimal,
    pub interest_rate_bps_daily: BigDecimal,
    pub required_dcr: BigDecimal,
    pub collateral_asset: String,
    pub status: FacilityStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FlashLiquidityDraw {
    pub draw_id: Uuid,
    pub facility_id: Uuid,
    pub parent_settlement_id: Uuid,
    pub corridor: String,
    pub draw_amount: BigDecimal,
    pub collateral_amount: BigDecimal,
    pub collateral_asset: String,
    pub escrow_account_hash: Option<String>,
    pub lock_xdr_signature: Option<String>,
    pub status: DrawStatus,
    pub repayment_due_at: DateTime<Utc>,
    pub repaid_at: Option<DateTime<Utc>>,
    pub interest_accrued: BigDecimal,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CollateralHealthLog {
    pub log_id: Uuid,
    pub draw_id: Uuid,
    pub collateral_value_usd: BigDecimal,
    pub debt_amount_usd: BigDecimal,
    pub health_factor: BigDecimal,
    pub near_liquidation: bool,
    pub circuit_breaker_action: Option<String>,
    pub evaluated_at: DateTime<Utc>,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FlashDrawRequest {
    pub parent_settlement_id: Uuid,
    pub corridor: String,
    pub required_amount: BigDecimal,
}

/// Result of a credit evaluation.
#[derive(Debug, Clone)]
pub struct CreditEvaluation {
    pub facility_id: Uuid,
    pub draw_amount: BigDecimal,
    pub collateral_required: BigDecimal,
    pub collateral_asset: String,
    pub interest_rate_bps_daily: BigDecimal,
}
