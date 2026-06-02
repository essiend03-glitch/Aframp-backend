//! #487 Smart Order Routing — data models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

// ── Enums ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "venue_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum VenueType {
    RegionalBank,
    StellarAmm,
    Mto,
    Cex,
    Dex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "venue_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum VenueStatus {
    Active,
    Degraded,
    Offline,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "sor_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SorStatus {
    Pending,
    Routing,
    Partial,
    Completed,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "child_order_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ChildOrderStatus {
    Pending,
    Submitted,
    Filled,
    PartialFill,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "rebalancing_trigger", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RebalancingTrigger {
    ThresholdBreach,
    Scheduled,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "rebalance_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RebalanceStatus {
    Initiated,
    InProgress,
    Completed,
    Failed,
}

// ── DB rows ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LiquidityVenue {
    pub venue_id: Uuid,
    pub name: String,
    pub venue_type: VenueType,
    pub status: VenueStatus,
    pub api_endpoint: String,
    pub supported_currencies: Vec<String>,
    pub daily_volume_limit: BigDecimal,
    pub used_volume_today: BigDecimal,
    pub execution_fee_bps: BigDecimal,
    pub spread_bps: BigDecimal,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SmartOrderExecution {
    pub execution_id: Uuid,
    pub parent_transaction_id: Uuid,
    pub correlation_tag: String,
    pub source_currency: String,
    pub target_currency: String,
    pub total_amount: BigDecimal,
    pub status: SorStatus,
    pub routing_plan: serde_json::Value,
    pub realized_slippage_bps: Option<BigDecimal>,
    pub path_calc_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SorChildOrder {
    pub child_order_id: Uuid,
    pub execution_id: Uuid,
    pub venue_id: Uuid,
    pub allocation_pct: BigDecimal,
    pub allocated_amount: BigDecimal,
    pub filled_amount: BigDecimal,
    pub status: ChildOrderStatus,
    pub venue_order_ref: Option<String>,
    pub slippage_bps: Option<BigDecimal>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub failed_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TreasuryRebalancingRule {
    pub rule_id: Uuid,
    pub currency_code: String,
    pub min_inventory_pct: BigDecimal,
    pub target_inventory_pct: BigDecimal,
    pub max_inventory_pct: BigDecimal,
    pub trigger_type: RebalancingTrigger,
    pub schedule_cron: Option<String>,
    pub enabled: bool,
    pub last_triggered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── In-memory routing types ───────────────────────────────────────────────────

/// A single edge in the routing graph: source → target via a venue.
#[derive(Debug, Clone)]
pub struct RouteEdge {
    pub venue_id: Uuid,
    pub venue_name: String,
    pub venue_type: VenueType,
    pub source_currency: String,
    pub target_currency: String,
    /// Total cost in basis points (spread + execution fee)
    pub cost_bps: f64,
    /// Available depth in source currency
    pub available_depth: BigDecimal,
}

/// Result of the pathfinder: an ordered list of edges forming the cheapest path.
#[derive(Debug, Clone)]
pub struct RoutePath {
    pub edges: Vec<RouteEdge>,
    /// Cumulative cost in basis points
    pub total_cost_bps: f64,
}

/// A single slice of a split order.
#[derive(Debug, Clone, Serialize)]
pub struct OrderSlice {
    pub venue_id: Uuid,
    pub venue_name: String,
    pub allocation_pct: f64,
    pub amount: BigDecimal,
}

/// Request to route an order.
#[derive(Debug, Clone)]
pub struct RouteOrderRequest {
    pub parent_transaction_id: Uuid,
    pub source_currency: String,
    pub target_currency: String,
    pub amount: BigDecimal,
    /// Hard slippage limit in basis points (e.g. 25 = 0.25 %)
    pub max_slippage_bps: f64,
}
