use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

/// Status of a collateralized lending position
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "lending_position_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LendingPositionStatus {
    Active,
    AtRisk,
    Liquidated,
    Repaid,
    Closed,
}

/// Type of collateral adjustment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "collateral_adjustment_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CollateralAdjustmentType {
    Deposit,
    Withdrawal,
}

/// A collateralized lending position
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LendingPosition {
    pub position_id: Uuid,
    pub wallet_id: Uuid,
    pub lending_protocol_id: String,
    pub collateral_asset_code: String,
    pub collateral_amount: BigDecimal,
    pub collateral_value_fiat: BigDecimal,
    pub borrowed_asset_code: String,
    pub borrowed_amount: BigDecimal,
    pub borrowed_value_fiat: BigDecimal,
    pub collateral_ratio: BigDecimal,
    pub liquidation_threshold_ratio: BigDecimal,
    pub health_factor: BigDecimal,
    pub interest_rate: BigDecimal,
    pub interest_accrued: BigDecimal,
    pub status: LendingPositionStatus,
    pub opened_at: DateTime<Utc>,
    pub last_health_check_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Loan repayment record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LoanRepayment {
    pub repayment_id: Uuid,
    pub position_id: Uuid,
    pub repayment_amount: BigDecimal,
    pub repayment_asset: String,
    pub interest_paid: BigDecimal,
    pub principal_repaid: BigDecimal,
    pub remaining_balance: BigDecimal,
    pub transaction_reference: String,
    pub repaid_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Collateral adjustment record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CollateralAdjustment {
    pub adjustment_id: Uuid,
    pub position_id: Uuid,
    pub adjustment_type: CollateralAdjustmentType,
    pub adjustment_amount: BigDecimal,
    pub pre_adjustment_collateral: BigDecimal,
    pub post_adjustment_collateral: BigDecimal,
    pub pre_adjustment_health_factor: BigDecimal,
    pub post_adjustment_health_factor: BigDecimal,
    pub adjusted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Liquidation event record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LiquidationEvent {
    pub liquidation_id: Uuid,
    pub position_id: Uuid,
    pub trigger_health_factor: BigDecimal,
    pub liquidated_collateral_amount: BigDecimal,
    pub liquidated_collateral_value: BigDecimal,
    pub repaid_debt_amount: BigDecimal,
    pub liquidation_penalty_amount: BigDecimal,
    pub liquidator_address: String,
    pub liquidated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Request to open a new lending position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenPositionRequest {
    pub wallet_id: Uuid,
    pub lending_protocol_id: String,
    pub collateral_asset_code: String,
    pub collateral_amount: BigDecimal,
    pub borrowed_asset_code: String,
    pub borrowed_amount: BigDecimal,
    pub liquidation_threshold_ratio: BigDecimal,
    pub interest_rate: BigDecimal,
}

/// Request to repay a loan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepayLoanRequest {
    pub position_id: Uuid,
    pub repayment_amount: BigDecimal,
    pub repayment_asset: String,
    pub transaction_reference: String,
}

/// Request to adjust collateral
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustCollateralRequest {
    pub position_id: Uuid,
    pub adjustment_type: CollateralAdjustmentType,
    pub adjustment_amount: BigDecimal,
}
