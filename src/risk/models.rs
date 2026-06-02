//! Data models for the Liquidity Risk & Circuit Breaker Engine — Issue #494.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

// ── Isolation scope ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IsolationScope {
    Global,
    Corridor,
    Tenant,
    Bank,
}

impl std::fmt::Display for IsolationScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global => write!(f, "GLOBAL"),
            Self::Corridor => write!(f, "CORRIDOR"),
            Self::Tenant => write!(f, "TENANT"),
            Self::Bank => write!(f, "BANK"),
        }
    }
}

// ── Risk corridor profile ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RiskCorridorProfile {
    pub id: Uuid,
    pub corridor_id: String,
    pub max_volatility_sigma: BigDecimal, // e.g. 3.0
    pub max_settlement_float: BigDecimal,
    pub velocity_cap_per_hour: BigDecimal,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Circuit breaker event ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CircuitBreakerEvent {
    pub id: Uuid,
    pub corridor_id: String,
    pub scope: String,
    pub trigger_metric: String,
    pub trigger_value: BigDecimal,
    pub trigger_threshold: BigDecimal,
    pub status: String, // "active" | "released"
    pub release_approvals: serde_json::Value, // [{officer, sig, at}]
    pub triggered_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub audit_hash: String, // SHA-256 of trigger payload
}

// ── API heartbeat ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApiHealthHeartbeat {
    pub id: Uuid,
    pub bank_id: String,
    pub latency_ms: i32,
    pub status_code: i32,
    pub error: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ReleaseApprovalRequest {
    pub officer_id: String,
    pub signature: String, // base64-encoded Ed25519 sig over event_id
}

#[derive(Debug, Serialize)]
pub struct RiskDashboard {
    pub profiles: Vec<RiskCorridorProfile>,
    pub active_circuit_breakers: Vec<CircuitBreakerEvent>,
    pub recent_heartbeats: Vec<ApiHealthHeartbeat>,
}
