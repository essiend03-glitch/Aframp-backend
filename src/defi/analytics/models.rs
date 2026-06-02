use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

// ── Platform Snapshot ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DefiPlatformSnapshot {
    pub snapshot_id: Uuid,
    pub snapshot_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_value_locked: BigDecimal,
    pub total_yield_distributed: BigDecimal,
    pub weighted_avg_yield_rate: f64,
    pub total_amm_liquidity: BigDecimal,
    pub total_collateral_locked: BigDecimal,
    pub total_outstanding_loans: BigDecimal,
    pub active_savings_positions: i64,
    pub active_amm_positions: i64,
    pub active_lending_positions: i64,
    pub platform_defi_revenue: BigDecimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PlatformSummaryResponse {
    pub current: DefiPlatformSnapshot,
    pub tvl_delta_pct: f64,
    pub yield_delta_pct: f64,
    pub revenue_delta_pct: f64,
}

// ── Strategy Snapshot ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DefiStrategySnapshot {
    pub snapshot_id: Uuid,
    pub strategy_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_allocated: BigDecimal,
    pub yield_earned: BigDecimal,
    pub effective_yield_rate: f64,
    pub max_drawdown: f64,
    pub risk_adjusted_return: f64,
    pub rebalancing_event_count: i32,
    pub protocol_contributions: serde_json::Value,
    pub benchmark_yield_rate: f64,
    pub benchmark_delta: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct StrategyAnalyticsResponse {
    pub strategy_id: Uuid,
    pub strategy_name: String,
    pub snapshots: Vec<DefiStrategySnapshot>,
    pub trend: YieldTrend,
    pub rank_by_risk_adjusted_return: u32,
}

#[derive(Debug, Serialize)]
pub struct YieldAttributionResponse {
    pub strategy_id: Uuid,
    pub protocol_contributions: HashMap<String, f64>,
    pub period_contributions: Vec<PeriodContribution>,
}

#[derive(Debug, Serialize)]
pub struct PeriodContribution {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub yield_earned: BigDecimal,
    pub pct_of_total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum YieldTrend {
    Improving,
    Stable,
    Declining,
}

// ── Protocol Snapshot ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DefiProtocolSnapshot {
    pub snapshot_id: Uuid,
    pub protocol_id: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub platform_exposure: BigDecimal,
    pub yield_earned: BigDecimal,
    pub fee_income: BigDecimal,
    pub impermanent_loss: BigDecimal,
    pub health_score: f64,
    pub uptime_pct: f64,
    pub capital_efficiency: f64,
    pub created_at: DateTime<Utc>,
}

// ── AMM Pool Snapshot ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DefiAmmPoolSnapshot {
    pub snapshot_id: Uuid,
    pub pool_id: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub trading_volume: BigDecimal,
    pub fee_income: BigDecimal,
    pub impermanent_loss: BigDecimal,
    pub hold_strategy_return: BigDecimal,
    pub actual_yield: BigDecimal,
    pub capital_efficiency: f64,
    pub price_range_coverage_pct: f64,
    pub created_at: DateTime<Utc>,
}

// ── Lending Snapshot ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DefiLendingSnapshot {
    pub snapshot_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_collateral: BigDecimal,
    pub total_outstanding_loans: BigDecimal,
    pub avg_loan_to_value_ratio: f64,
    pub avg_health_factor: f64,
    pub liquidation_count: i32,
    pub liquidation_rate: f64,
    pub interest_income: BigDecimal,
    pub unique_borrowers: i32,
    pub avg_loan_size: BigDecimal,
    pub created_at: DateTime<Utc>,
}

// ── User Snapshot ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DefiUserSnapshot {
    pub snapshot_id: Uuid,
    pub wallet_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_deposited_savings: BigDecimal,
    pub total_yield_earned: BigDecimal,
    pub net_yield_rate: f64,
    pub total_collateral_locked: BigDecimal,
    pub outstanding_loan_balance: BigDecimal,
    pub net_defi_position_value: BigDecimal,
    pub product_usage: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ── Reports ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DefiAnalyticsReport {
    pub report_id: Uuid,
    pub report_type: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub status: String,
    pub report_data: Option<serde_json::Value>,
    pub download_url: Option<String>,
    pub generated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── Export ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub date_range_start: DateTime<Utc>,
    pub date_range_end: DateTime<Utc>,
    pub metric_set: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub export_id: Uuid,
    pub status: String,
    pub message: String,
}

// ── Query Params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HistoryParams {
    pub granularity: Option<String>, // "daily" | "weekly" | "monthly"
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PeriodParams {
    pub limit: Option<i64>,
}
