//! Sanctions screening data models — #419

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which watchlist produced the hit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchlistSource {
    Ofac,
    Un,
    Eu,
    Local,
    Other(String),
}

impl std::fmt::Display for WatchlistSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ofac => write!(f, "OFAC"),
            Self::Un => write!(f, "UN"),
            Self::Eu => write!(f, "EU"),
            Self::Local => write!(f, "LOCAL"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

/// A single match against a watchlist entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanctionsMatch {
    pub source: WatchlistSource,
    pub matched_name: String,
    /// Similarity score 0.0–1.0 (1.0 = exact)
    pub score: f64,
    pub entity_id: String,
}

/// Outcome of screening one entity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreeningOutcome {
    /// No hits — transaction may proceed
    Clear,
    /// One or more hits — transaction must be blocked
    Hit,
    /// Provider unreachable — fail-closed, transaction paused
    ProviderError,
}

/// Input to the sanctions screener
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreeningRequest {
    pub transaction_id: Uuid,
    pub sender_id: String,
    pub sender_name: String,
    pub receiver_id: String,
    pub receiver_name: String,
    /// Optional intermediary (e.g. correspondent bank)
    pub intermediary_name: Option<String>,
}

/// Full result returned by the screener
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreeningResult {
    pub transaction_id: Uuid,
    pub outcome: ScreeningOutcome,
    pub matches: Vec<SanctionsMatch>,
    pub screened_at: DateTime<Utc>,
    /// Wall-clock latency of the screening call in milliseconds
    pub latency_ms: u64,
}

impl ScreeningResult {
    pub fn is_blocked(&self) -> bool {
        matches!(self.outcome, ScreeningOutcome::Hit | ScreeningOutcome::ProviderError)
    }
}

/// Immutable audit log entry written for every screening call
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScreeningLogEntry {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub outcome: String,
    pub matches_json: serde_json::Value,
    pub latency_ms: i64,
    pub created_at: DateTime<Utc>,
}

/// Dual-auth bypass record — requires two distinct approvers
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BypassRequest {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub reason: String,
    pub first_approver_id: String,
    pub second_approver_id: Option<String>,
    pub approved: bool,
    pub created_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
}
