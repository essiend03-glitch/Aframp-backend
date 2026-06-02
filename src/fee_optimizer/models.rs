//! #490 Gas & Fee Optimization Engine — data models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Enums ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "chain_network", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ChainNetwork {
    Stellar,
    Ethereum,
    Solana,
    Polygon,
    Arbitrum,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "urgency_window", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UrgencyWindow {
    Immediate,
    OneMins,
    FiveMins,
    ThirtyMins,
    BestEffort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "gas_log_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum GasLogStatus {
    Pending,
    Submitted,
    Confirmed,
    Bumped,
    Dropped,
    Failed,
}

// ── DB rows ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NetworkFeeSnapshot {
    pub snapshot_id: Uuid,
    pub network: ChainNetwork,
    pub base_fee: sqlx::types::BigDecimal,
    pub priority_fee: sqlx::types::BigDecimal,
    pub ema_base_fee: sqlx::types::BigDecimal,
    pub ema_priority_fee: sqlx::types::BigDecimal,
    pub rpc_provider: String,
    pub block_reference: Option<i64>,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FeeOptimizationPolicy {
    pub policy_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub network: ChainNetwork,
    pub urgency: UrgencyWindow,
    pub max_fee_cap: sqlx::types::BigDecimal,
    pub fee_multiplier: sqlx::types::BigDecimal,
    pub congestion_halt_threshold: sqlx::types::BigDecimal,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExecutionGasLog {
    pub gas_log_id: Uuid,
    pub parent_tx_id: Uuid,
    pub network: ChainNetwork,
    pub urgency: UrgencyWindow,
    pub estimated_fee: sqlx::types::BigDecimal,
    pub actual_fee: Option<sqlx::types::BigDecimal>,
    pub bump_count: i32,
    pub tx_hash: Option<String>,
    pub nonce_or_sequence: Option<i64>,
    pub status: GasLogStatus,
    pub submitted_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub last_bumped_at: Option<DateTime<Utc>>,
}

// ── In-memory types ───────────────────────────────────────────────────────────

/// Current optimized fee parameters for a network, ready to sign.
#[derive(Debug, Clone, Serialize)]
pub struct OptimizedFeeParams {
    pub network: ChainNetwork,
    pub max_fee_per_gas: u128,       // Wei / Stroop / Lamport
    pub max_priority_fee_per_gas: u128,
    pub urgency: UrgencyWindow,
    pub estimated_at: DateTime<Utc>,
}

/// EMA state per network.
#[derive(Debug, Clone)]
pub struct EmaState {
    pub ema_base: f64,
    pub ema_priority: f64,
    /// EMA smoothing factor α (0 < α ≤ 1).
    pub alpha: f64,
}

impl EmaState {
    pub fn new(alpha: f64) -> Self {
        Self { ema_base: 0.0, ema_priority: 0.0, alpha }
    }

    /// Update EMA with a new observation.
    pub fn update(&mut self, base: f64, priority: f64) {
        if self.ema_base == 0.0 {
            // Seed with first observation
            self.ema_base = base;
            self.ema_priority = priority;
        } else {
            self.ema_base = self.alpha * base + (1.0 - self.alpha) * self.ema_base;
            self.ema_priority = self.alpha * priority + (1.0 - self.alpha) * self.ema_priority;
        }
    }
}
