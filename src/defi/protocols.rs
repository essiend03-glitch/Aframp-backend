use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::AppError;

/// Core DeFi protocol abstraction trait
/// 
/// This trait provides a common interface for interacting with any DeFi protocol,
/// allowing the platform to add, modify, or remove protocol integrations without
/// changing the core DeFi integration framework.
#[async_trait]
pub trait DeFiProtocol: Send + Sync {
    /// Protocol identifier (e.g., "stellar_dex", "stellar_amm")
    fn protocol_id(&self) -> &str;

    /// Protocol name for display purposes
    fn protocol_name(&self) -> &str;

    /// Current risk tier classification
    fn risk_tier(&self) -> RiskTier;

    /// Deposit assets into the protocol
    async fn deposit(
        &self,
        amount: BigDecimal,
        asset_code: &str,
        slippage_tolerance: f64,
    ) -> Result<DeFiPosition, AppError>;

    /// Withdraw assets from the protocol
    async fn withdraw(
        &self,
        position_id: Uuid,
        amount: BigDecimal,
        slippage_tolerance: f64,
    ) -> Result<DeFiWithdrawalResult, AppError>;

    /// Fetch current position value and metrics
    async fn get_position(&self, position_id: Uuid) -> Result<DeFiPosition, AppError>;

    /// Fetch current yield rate for the protocol
    async fn get_yield_rate(&self, asset_code: &str) -> Result<f64, AppError>;

    /// Fetch protocol health metrics
    async fn get_health_metrics(&self) -> Result<ProtocolHealthMetrics, AppError>;

    /// Execute a swap operation (if supported)
    async fn swap(
        &self,
        from_asset: &str,
        to_asset: &str,
        amount: BigDecimal,
        slippage_tolerance: f64,
    ) -> Result<DeFiSwapResult, AppError>;

    /// Check if the protocol supports a specific asset pair
    fn supports_asset_pair(&self, from_asset: &str, to_asset: &str) -> bool;

    /// Get minimum deposit amount for the protocol
    fn min_deposit_amount(&self, asset_code: &str) -> BigDecimal;

    /// Get maximum deposit amount for the protocol
    fn max_deposit_amount(&self, asset_code: &str) -> BigDecimal;
}

/// Risk tier classification for DeFi protocols
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "risk_tier", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum RiskTier {
    /// Battle-tested, multi-audited, high TVL, strong track record
    Tier1,
    /// Established, audited, moderate TVL, good track record
    Tier2,
    /// Newer, limited track record - platform funds not permitted
    Tier3,
}

/// DeFi position representing platform exposure in a protocol
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeFiPosition {
    pub position_id: Uuid,
    pub protocol_id: String,
    pub asset_code: String,
    pub deposited_amount: BigDecimal,
    pub current_value: BigDecimal,
    pub yield_earned: BigDecimal,
    pub effective_yield_rate: f64,
    pub position_opened_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub position_status: PositionStatus,
}

/// Position status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "position_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PositionStatus {
    Active,
    Withdrawing,
    Closed,
    EmergencyWithdrawal,
}

/// Result of a withdrawal operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeFiWithdrawalResult {
    pub position_id: Uuid,
    pub withdrawn_amount: BigDecimal,
    pub gross_value: BigDecimal,
    pub fees_paid: BigDecimal,
    pub net_value: BigDecimal,
    pub realized_yield: BigDecimal,
    pub transaction_hash: String,
    pub completed_at: DateTime<Utc>,
}

/// Result of a swap operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeFiSwapResult {
    pub from_asset: String,
    pub to_asset: String,
    pub input_amount: BigDecimal,
    pub output_amount: BigDecimal,
    pub slippage_pct: f64,
    pub fees_paid: BigDecimal,
    pub transaction_hash: String,
    pub completed_at: DateTime<Utc>,
}

/// Protocol health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolHealthMetrics {
    pub protocol_id: String,
    pub total_value_locked: BigDecimal,
    pub tvl_change_24h: f64,
    pub volume_24h: BigDecimal,
    pub active_positions: u64,
    pub average_yield_rate: f64,
    pub health_score: f64, // 0.0 to 1.0
    pub last_updated_at: DateTime<Utc>,
    pub additional_metrics: HashMap<String, serde_json::Value>,
}

/// Protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProtocolConfig {
    pub protocol_id: String,
    pub protocol_name: String,
    pub protocol_type: ProtocolType,
    pub risk_tier: RiskTier,
    pub is_active: bool,
    pub max_exposure_percentage: f64,
    pub max_single_transaction_amount: BigDecimal,
    pub min_deposit_amount: BigDecimal,
    pub max_deposit_amount: BigDecimal,
    pub default_slippage_tolerance: f64,
    pub health_check_interval_secs: u64,
    pub evaluation_scores: EvaluationScores,
    pub governance_status: GovernanceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Protocol type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "protocol_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ProtocolType {
    Dex,
    Amm,
    Lending,
    LiquidityMining,
    YieldFarming,
}

/// Evaluation scores from protocol evaluation framework
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EvaluationScores {
    pub tvl_score: f64,
    pub age_score: f64,
    pub audit_score: f64,
    pub team_score: f64,
    pub codebase_score: f64,
    pub governance_score: f64,
    pub compliance_score: f64,
    pub ecosystem_score: f64,
    pub total_score: f64,
}

/// Governance status for protocol approval
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "governance_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum GovernanceStatus {
    Pending,
    Approved,
    Rejected,
    Suspended,
}

/// Yield strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct YieldStrategy {
    pub strategy_id: Uuid,
    pub strategy_name: String,
    pub description: String,
    pub strategy_type: StrategyType,
    pub target_yield_rate: f64,
    pub min_acceptable_yield_rate: f64,
    pub max_acceptable_risk_score: f64,
    pub total_allocated_amount: BigDecimal,
    pub max_allocation_limit: BigDecimal,
    pub rebalancing_frequency_secs: u64,
    pub rebalancing_trigger_conditions: RebalancingTriggers,
    pub strategy_status: StrategyStatus,
    pub governance_approval_record: GovernanceApprovalRecord,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Strategy type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "strategy_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StrategyType {
    SingleProtocol,
    MultiProtocol,
    DynamicAllocation,
}

/// Strategy status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "strategy_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StrategyStatus {
    Draft,
    PendingApproval,
    Active,
    Paused,
    Deprecated,
}

/// Rebalancing trigger conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalancingTriggers {
    pub time_based_interval_secs: u64,
    pub drift_tolerance_pct: f64,
    pub yield_rate_deviation_pct: f64,
    pub performance_based_enabled: bool,
}

/// Governance approval record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GovernanceApprovalRecord {
    pub record_id: Uuid,
    pub strategy_id: Uuid,
    pub submitted_by: String,
    pub submitted_at: DateTime<Utc>,
    pub required_approvals: usize,
    pub received_approvals: usize,
    pub approval_status: GovernanceStatus,
    pub approvals: Vec<GovernanceApproval>,
    pub rejection_reason: Option<String>,
}

/// Individual governance approval
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GovernanceApproval {
    pub approval_id: Uuid,
    pub committee_member: String,
    pub approved_at: DateTime<Utc>,
    pub justification: String,
    pub approval_type: ApprovalType,
}

/// Approval type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "approval_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ApprovalType {
    Approve,
    Reject,
}

/// Strategy allocation configuration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StrategyAllocation {
    pub allocation_id: Uuid,
    pub strategy_id: Uuid,
    pub protocol_id: String,
    pub target_allocation_percentage: f64,
    pub current_allocation_amount: BigDecimal,
    pub min_allocation_percentage: f64,
    pub max_allocation_percentage: f64,
    pub last_rebalanced_at: DateTime<Utc>,
    pub allocation_status: AllocationStatus,
}

/// Allocation status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "allocation_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AllocationStatus {
    Active,
    Rebalancing,
    Paused,
    Closed,
}

/// Strategy performance record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StrategyPerformance {
    pub performance_id: Uuid,
    pub strategy_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub opening_allocation: BigDecimal,
    pub closing_allocation: BigDecimal,
    pub yield_earned: BigDecimal,
    pub effective_yield_rate: f64,
    pub max_drawdown: f64,
    pub risk_score_at_end: f64,
    pub created_at: DateTime<Utc>,
}

/// Strategy risk parameters
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StrategyRiskParameters {
    pub parameter_id: Uuid,
    pub strategy_id: Uuid,
    pub max_single_protocol_exposure_pct: f64,
    pub max_correlation_between_protocols: f64,
    pub max_acceptable_impermanent_loss_pct: f64,
    pub circuit_breaker_tvl_drop_threshold: f64,
    pub emergency_withdrawal_trigger_conditions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
