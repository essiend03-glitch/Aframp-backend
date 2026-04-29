use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

// Re-export from submodules for convenience
pub use super::protocols::*;
pub use super::evaluation::*;
pub use super::governance::*;
pub use super::risk_controls::*;

/// DeFi integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeFiConfig {
    pub max_treasury_exposure_pct: f64,
    pub max_single_protocol_exposure_pct: f64,
    pub max_single_transaction_amount: u64,
    pub default_slippage_tolerance: f64,
    pub circuit_breaker_tvl_drop_threshold: f64,
    pub circuit_breaker_tvl_drop_window_hours: i64,
    pub min_governance_approvals: usize,
    pub protocol_health_check_interval_secs: u64,
    pub treasury_exposure_check_interval_secs: u64,
    pub yield_rate_update_interval_secs: u64,
}

impl Default for DeFiConfig {
    fn default() -> Self {
        Self {
            max_treasury_exposure_pct: 30.0,
            max_single_protocol_exposure_pct: 10.0,
            max_single_transaction_amount: 1_000_000,
            default_slippage_tolerance: 0.01,
            circuit_breaker_tvl_drop_threshold: 0.20,
            circuit_breaker_tvl_drop_window_hours: 24,
            min_governance_approvals: 3,
            protocol_health_check_interval_secs: 300,
            treasury_exposure_check_interval_secs: 600,
            yield_rate_update_interval_secs: 3600,
        }
    }
}

/// Treasury allocation model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TreasuryAllocation {
    pub allocation_id: Uuid,
    pub protocol_id: String,
    pub allocation_type: TreasuryAllocationType,
    pub allocated_amount: BigDecimal,
    pub current_value: BigDecimal,
    pub yield_earned: BigDecimal,
    pub allocation_percentage: f64,
    pub allocated_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub status: TreasuryAllocationStatus,
}

/// Treasury allocation type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "treasury_allocation_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TreasuryAllocationType {
    YieldStrategy,
    LiquidityProvision,
    MarketMaking,
    Reserve,
}

/// Treasury allocation status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "treasury_allocation_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TreasuryAllocationStatus {
    Active,
    Rebalancing,
    Withdrawing,
    Closed,
}

/// Treasury exposure metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryExposureMetrics {
    pub total_treasury_value: BigDecimal,
    pub total_defi_exposure: BigDecimal,
    pub defi_exposure_percentage: f64,
    pub protocol_exposures: HashMap<String, ProtocolExposure>,
    pub risk_metrics: TreasuryRiskMetrics,
    pub last_updated_at: DateTime<Utc>,
}

/// Individual protocol exposure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolExposure {
    pub protocol_id: String,
    pub protocol_name: String,
    pub risk_tier: RiskTier,
    pub allocated_amount: BigDecimal,
    pub current_value: BigDecimal,
    pub exposure_percentage: f64,
    pub yield_earned: BigDecimal,
    pub position_count: u64,
}

/// Treasury risk metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryRiskMetrics {
    pub weighted_risk_score: f64,
    pub max_single_protocol_exposure_pct: f64,
    pub concentration_risk_score: f64,
    pub correlation_risk_score: f64,
    pub liquidity_risk_score: f64,
}

/// cNGN savings product configuration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CngnSavingsProduct {
    pub product_id: Uuid,
    pub product_name: String,
    pub description: String,
    pub product_type: SavingsProductType,
    pub minimum_deposit_amount: BigDecimal,
    pub maximum_deposit_amount: BigDecimal,
    pub lock_up_period_hours: i64,
    pub early_withdrawal_penalty_pct: f64,
    pub target_yield_rate: f64,
    pub yield_rate_source: String,
    pub underlying_strategy_id: Option<Uuid>,
    pub yield_rate_floor: Option<f64>,
    pub yield_rate_ceil: Option<f64>,
    pub product_status: String,
    pub risk_disclosure_version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Savings product type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "savings_product_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum SavingsProductType {
    Flexible,
    FixedTerm,
}

/// cNGN savings account
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CngnSavingsAccount {
    pub account_id: Uuid,
    pub wallet_id: Uuid,
    pub product_id: Uuid,
    pub deposited_amount: BigDecimal,
    pub current_balance: BigDecimal,
    pub accrued_yield_to_date: BigDecimal,
    pub current_yield_rate: f64,
    pub deposit_timestamp: DateTime<Utc>,
    pub last_yield_accrual_timestamp: DateTime<Utc>,
    pub withdrawal_eligibility_timestamp: DateTime<Utc>,
    pub account_status: SavingsAccountStatus,
    pub risk_disclosure_accepted_at: DateTime<Utc>,
    pub risk_disclosure_ip_address: Option<String>,
}

/// Savings account status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "savings_account_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum SavingsAccountStatus {
    Active,
    Withdrawing,
    Closed,
}

/// Yield accrual record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct YieldAccrualRecord {
    pub accrual_id: Uuid,
    pub account_id: Uuid,
    pub accrual_period_start: DateTime<Utc>,
    pub accrual_period_end: DateTime<Utc>,
    pub opening_balance: BigDecimal,
    pub yield_rate_applied: f64,
    pub yield_amount_earned: BigDecimal,
    pub accrual_timestamp: DateTime<Utc>,
}

/// Withdrawal request
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WithdrawalRequest {
    pub request_id: Uuid,
    pub account_id: Uuid,
    pub requested_amount: BigDecimal,
    pub withdrawal_type: WithdrawalType,
    pub early_withdrawal_flag: bool,
    pub penalty_amount: BigDecimal,
    pub net_withdrawal_amount: BigDecimal,
    pub request_timestamp: DateTime<Utc>,
    pub settlement_timestamp: Option<DateTime<Utc>>,
    pub status: String,
    pub transaction_hash: Option<String>,
}

/// Withdrawal type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "withdrawal_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum WithdrawalType {
    Full,
    Partial,
}

/// Stellar AMM pool information
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StellarAmmPool {
    pub pool_id: String,
    pub asset_a_code: String,
    pub asset_a_issuer: Option<String>,
    pub asset_b_code: String,
    pub asset_b_issuer: Option<String>,
    pub total_pool_shares: BigDecimal,
    pub asset_a_reserves: BigDecimal,
    pub asset_b_reserves: BigDecimal,
    pub current_price: BigDecimal,
    pub trading_fee_bps: i32,
    pub pool_status: AmmPoolStatus,
    pub tvl_24h_ago: Option<BigDecimal>,
    pub volume_24h: BigDecimal,
    pub fees_24h: BigDecimal,
    pub last_updated_at: DateTime<Utc>,
    pub discovered_at: DateTime<Utc>,
}

/// AMM pool status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "amm_pool_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AmmPoolStatus {
    Active,
    Inactive,
    Maintenance,
}

/// AMM liquidity position
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AmmLiquidityPosition {
    pub position_id: Uuid,
    pub pool_id: String,
    pub strategy_id: Option<Uuid>,
    pub shares_owned: BigDecimal,
    pub asset_a_deposited: BigDecimal,
    pub asset_b_deposited: BigDecimal,
    pub initial_share_price: BigDecimal,
    pub current_share_price: BigDecimal,
    pub unrealized_yield: BigDecimal,
    pub impermanent_loss: BigDecimal,
    pub fee_income_earned: BigDecimal,
    pub position_opened_at: DateTime<Utc>,
    pub last_valuation_at: DateTime<Utc>,
    pub position_status: String,
}

/// AMM position snapshot
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AmmPositionSnapshot {
    pub snapshot_id: Uuid,
    pub position_id: Uuid,
    pub pool_share_price: BigDecimal,
    pub asset_a_value: BigDecimal,
    pub asset_b_value: BigDecimal,
    pub total_position_value: BigDecimal,
    pub unrealized_yield: BigDecimal,
    pub impermanent_loss: BigDecimal,
    pub fee_income_earned: BigDecimal,
    pub snapshotted_at: DateTime<Utc>,
}

/// Yield rate history
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct YieldRateHistory {
    pub history_id: Uuid,
    pub product_id: Uuid,
    pub yield_rate: f64,
    pub rate_source: String,
    pub recorded_at: DateTime<Utc>,
}

/// Rebalancing event
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RebalancingEvent {
    pub event_id: Uuid,
    pub strategy_id: Uuid,
    pub trigger_reason: String,
    pub pre_rebalancing_allocations: serde_json::Value,
    pub post_rebalancing_allocations: serde_json::Value,
    pub transaction_details: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub error_message: Option<String>,
}

/// DeFi integration metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeFiMetrics {
    pub total_defi_exposure: BigDecimal,
    pub total_yield_earned: BigDecimal,
    pub active_strategies: u64,
    pub active_positions: u64,
    pub average_yield_rate: f64,
    pub circuit_breakers_tripped: u64,
    pub last_updated_at: DateTime<Utc>,
}

/// Strategy performance summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformanceSummary {
    pub strategy_id: Uuid,
    pub strategy_name: String,
    pub current_allocation: BigDecimal,
    pub total_yield_earned: BigDecimal,
    pub effective_yield_rate: f64,
    pub max_drawdown: f64,
    pub risk_score: f64,
    pub sharpe_ratio: f64,
    pub status: String,
    pub last_rebalanced: Option<DateTime<Utc>>,
}

/// Savings product performance summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsProductPerformanceSummary {
    pub product_id: Uuid,
    pub product_name: String,
    pub active_accounts: u64,
    pub total_deposits: BigDecimal,
    pub total_yield_accrued: BigDecimal,
    pub average_yield_rate: f64,
    pub product_status: String,
}

/// AMM pool performance summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmmPoolPerformanceSummary {
    pub pool_id: String,
    pub asset_pair: String,
    pub total_liquidity: BigDecimal,
    pub volume_24h: BigDecimal,
    pub fees_24h: BigDecimal,
    pub apr: f64,
    pub impermanent_loss_24h: f64,
    pub active_positions: u64,
    pub pool_status: String,
}

/// Risk disclosure acceptance
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RiskDisclosureAcceptance {
    pub acceptance_id: Uuid,
    pub user_id: String,
    pub product_id: Uuid,
    pub disclosure_version: String,
    pub accepted_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// DeFi operation request/response DTOs

#[derive(Debug, Deserialize)]
pub struct CreateStrategyRequest {
    pub strategy_name: String,
    pub description: String,
    pub strategy_type: StrategyType,
    pub target_yield_rate: f64,
    pub min_acceptable_yield_rate: f64,
    pub max_acceptable_risk_score: f64,
    pub max_allocation_limit: BigDecimal,
    pub rebalancing_frequency_secs: u64,
    pub rebalancing_triggers: RebalancingTriggers,
    pub allocations: Vec<CreateStrategyAllocationRequest>,
    pub risk_parameters: CreateStrategyRiskParametersRequest,
}

#[derive(Debug, Deserialize)]
pub struct CreateStrategyAllocationRequest {
    pub protocol_id: String,
    pub target_allocation_percentage: f64,
    pub min_allocation_percentage: f64,
    pub max_allocation_percentage: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateStrategyRiskParametersRequest {
    pub max_single_protocol_exposure_pct: f64,
    pub max_correlation_between_protocols: f64,
    pub max_acceptable_impermanent_loss_pct: f64,
    pub circuit_breaker_tvl_drop_threshold: f64,
    pub emergency_withdrawal_trigger_conditions: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct SubmitForApprovalRequest {
    pub justification: String,
}

#[derive(Debug, Deserialize)]
pub struct GovernanceApprovalRequest {
    pub approval_type: ApprovalType,
    pub justification: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSavingsAccountRequest {
    pub wallet_id: Uuid,
    pub product_id: Uuid,
    pub deposit_amount: BigDecimal,
    pub risk_disclosure_accepted: bool,
}

#[derive(Debug, Deserialize)]
pub struct DepositRequest {
    pub account_id: Uuid,
    pub amount: BigDecimal,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawRequest {
    pub account_id: Uuid,
    pub amount: BigDecimal,
    pub withdrawal_type: WithdrawalType,
}

#[derive(Debug, Deserialize)]
pub struct CreateAmmPositionRequest {
    pub pool_id: String,
    pub asset_a_amount: BigDecimal,
    pub asset_b_amount: BigDecimal,
    pub slippage_tolerance: f64,
}

#[derive(Debug, Deserialize)]
pub struct AmmSwapRequest {
    pub pool_id: String,
    pub from_asset: String,
    pub to_asset: String,
    pub amount: BigDecimal,
    pub slippage_tolerance: f64,
}

#[derive(Debug, Serialize)]
pub struct StrategyResponse {
    #[serde(flatten)]
    pub strategy: YieldStrategy,
    pub allocations: Vec<StrategyAllocation>,
    pub risk_parameters: Option<StrategyRiskParameters>,
    pub performance: Option<StrategyPerformance>,
    pub governance_status: Option<GovernanceApprovalRecord>,
}

#[derive(Debug, Serialize)]
pub struct SavingsAccountResponse {
    #[serde(flatten)]
    pub account: CngnSavingsAccount,
    pub product: CngnSavingsProduct,
    pub projected_yield: Option<ProjectedYield>,
}

#[derive(Debug, Serialize)]
pub struct ProjectedYield {
    pub period_days: u32,
    pub opening_balance: BigDecimal,
    pub projected_yield_amount: BigDecimal,
    pub projected_yield_rate: f64,
    pub projected_end_balance: BigDecimal,
}

#[derive(Debug, Serialize)]
pub struct AmmPoolResponse {
    #[serde(flatten)]
    pub pool: StellarAmmPool,
    pub platform_position: Option<AmmLiquidityPosition>,
    pub performance_metrics: Option<AmmPoolPerformanceSummary>,
}

#[derive(Debug, Serialize)]
pub struct DeFiOverviewResponse {
    pub total_exposure: BigDecimal,
    pub total_yield: BigDecimal,
    pub active_strategies: u64,
    pub active_positions: u64,
    pub average_yield_rate: f64,
    pub risk_metrics: TreasuryRiskMetrics,
    pub protocol_breakdown: Vec<ProtocolExposure>,
    pub strategy_breakdown: Vec<StrategyPerformanceSummary>,
    pub savings_breakdown: Vec<SavingsProductPerformanceSummary>,
}

#[derive(Debug, Serialize)]
pub struct CircuitBreakerStatusResponse {
    pub protocol_id: String,
    pub protocol_name: String,
    pub is_tripped: bool,
    pub last_trip: Option<CircuitBreakerTrip>,
    can_reset: bool,
    pub reset_requirements: Option<String>,
}
