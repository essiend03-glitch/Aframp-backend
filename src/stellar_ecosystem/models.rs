//! Data models for Stellar Ecosystem Partner Integration (Issue #470).

#[cfg(feature = "database")]
use chrono::{DateTime, Utc};
#[cfg(feature = "database")]
use rust_decimal::Decimal;
#[cfg(feature = "database")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "database")]
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Anchor connection
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AnchorConnection {
    pub id: Uuid,
    pub domain: String,
    pub display_name: String,
    pub status: String,
    pub supported_assets: Vec<String>,
    pub sep24_enabled: bool,
    pub sep31_enabled: bool,
    pub signing_key: Option<String>,
    pub jwt_token: Option<String>,
    pub jwt_expires_at: Option<DateTime<Utc>>,
    pub horizon_url: Option<String>,
    pub total_transfers: i64,
    pub total_volume_usd: Decimal,
    pub last_connected_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAnchorConnectionRequest {
    pub domain: String,
    pub display_name: String,
    pub supported_assets: Vec<String>,
    pub sep24_enabled: bool,
    pub sep31_enabled: bool,
    pub signing_key: Option<String>,
    pub horizon_url: Option<String>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAnchorConnectionRequest {
    pub display_name: Option<String>,
    pub status: Option<String>,
    pub supported_assets: Option<Vec<String>>,
    pub sep24_enabled: Option<bool>,
    pub sep31_enabled: Option<bool>,
    pub signing_key: Option<String>,
    pub horizon_url: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// DEX order book snapshot
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DexOrderBookSnapshot {
    pub id: Uuid,
    pub base_asset: String,
    pub counter_asset: String,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub mid_price: Option<Decimal>,
    pub spread_pct: Option<Decimal>,
    pub bids: serde_json::Value,
    pub asks: serde_json::Value,
    pub depth_1pct_base: Decimal,
    pub depth_1pct_counter: Decimal,
    pub snapshotted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: Decimal,
    pub amount: Decimal,
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-anchor transfer
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CrossAnchorTransfer {
    pub id: Uuid,
    pub reference_id: String,
    pub receiving_anchor_id: Uuid,
    pub sep31_transaction_id: Option<String>,
    pub compliance_tracking_id: Option<String>,
    pub status: String,
    pub send_asset: String,
    pub receive_asset: String,
    pub send_amount: Decimal,
    pub receive_amount: Option<Decimal>,
    pub execution_spread: Option<Decimal>,
    pub stellar_tx_hash: Option<String>,
    pub stellar_tx_xdr: Option<String>,
    pub stellar_ledger: Option<i64>,
    pub sender_account: String,
    pub receiver_account: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiateTransferRequest {
    pub receiving_anchor_domain: String,
    pub send_asset: String,
    pub receive_asset: String,
    pub send_amount: Decimal,
    pub sender_account: String,
    pub receiver_account: Option<String>,
    /// Maximum acceptable slippage as a fraction (e.g. 0.005 = 0.5%)
    pub max_slippage: Option<Decimal>,
}

// ─────────────────────────────────────────────────────────────────────────────
// SEP-24 / SEP-31 protocol types
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sep24DepositRequest {
    pub asset_code: String,
    pub asset_issuer: Option<String>,
    pub account: String,
    pub amount: Option<Decimal>,
    pub memo: Option<String>,
    pub memo_type: Option<String>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sep24WithdrawRequest {
    pub asset_code: String,
    pub asset_issuer: Option<String>,
    pub account: String,
    pub amount: Option<Decimal>,
    pub dest: Option<String>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sep24InteractiveResponse {
    pub transaction_id: String,
    pub url: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sep31SendRequest {
    pub amount: Decimal,
    pub asset_code: String,
    pub asset_issuer: Option<String>,
    pub destination_asset: Option<String>,
    pub sender_id: Option<String>,
    pub receiver_id: Option<String>,
    pub fields: Option<serde_json::Value>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sep31SendResponse {
    pub id: String,
    pub stellar_account_id: String,
    pub stellar_memo: Option<String>,
    pub stellar_memo_type: Option<String>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sep31TransactionStatus {
    pub id: String,
    pub status: String,
    pub amount_in: Option<String>,
    pub amount_out: Option<String>,
    pub stellar_transaction_id: Option<String>,
    pub message: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// DEX pathfinding
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathfindingRequest {
    pub source_asset: String,
    pub destination_asset: String,
    pub source_amount: Option<Decimal>,
    pub destination_amount: Option<Decimal>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathfindingResult {
    pub source_asset: String,
    pub source_amount: Decimal,
    pub destination_asset: String,
    pub destination_amount: Decimal,
    pub path: Vec<String>,
    /// Computed spread as a fraction
    pub spread: Decimal,
    /// Whether slippage is within configured tolerance
    pub within_tolerance: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin configuration
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    /// Maximum allowed slippage fraction (e.g. 0.005 = 0.5%)
    pub max_slippage: Decimal,
    /// Minimum liquidity depth required (in base asset units)
    pub min_liquidity_depth: Decimal,
    /// Asset pairs to monitor
    pub monitored_pairs: Vec<AssetPair>,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPair {
    pub base_asset: String,
    pub counter_asset: String,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDexConfigRequest {
    pub max_slippage: Option<Decimal>,
    pub min_liquidity_depth: Option<Decimal>,
    pub monitored_pairs: Option<Vec<AssetPair>>,
}
