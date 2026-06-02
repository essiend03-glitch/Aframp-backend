//! Data models for the Sanctions Screening Engine — Issue #495.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

// ── Watchlist entry ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ComplianceWatchlistEntry {
    pub id: Uuid,
    pub full_name: String,
    pub aliases: Vec<String>,
    pub passport_numbers: Vec<String>,
    pub wallet_addresses: Vec<String>,
    pub list_source: String, // "OFAC" | "EU" | "UN" | "LOCAL"
    pub match_threshold: BigDecimal, // 0.85 default
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Sanctions match record ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SanctionsMatchRecord {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub watchlist_entry_id: Uuid,
    pub matched_field: String,
    pub match_score: BigDecimal,
    pub status: String, // "HELD_FOR_REVIEW" | "CLEARED" | "CONFIRMED_BREACH"
    pub reviewer_id: Option<String>,
    pub reviewer_notes: Option<String>,
    pub compliance_cert_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Behavioral risk analytics ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BehavioralRiskRecord {
    pub id: Uuid,
    pub account_id: String,
    pub window_1h_volume: BigDecimal,
    pub window_24h_volume: BigDecimal,
    pub window_7d_volume: BigDecimal,
    pub tx_count_1h: i32,
    pub velocity_score: BigDecimal, // 0-100
    pub smurfing_flag: bool,
    pub recorded_at: DateTime<Utc>,
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ReviewDecisionRequest {
    pub decision: String, // "CLEARED" | "CONFIRMED_BREACH"
    pub reviewer_id: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ComplianceDashboard {
    pub pending_reviews: Vec<SanctionsMatchRecord>,
    pub recent_matches: Vec<SanctionsMatchRecord>,
    pub watchlist_count: i64,
    pub last_sync_at: Option<DateTime<Utc>>,
}
