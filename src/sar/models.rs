//! SAR data models and state machine

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SAR lifecycle state machine:
/// Draft → PendingReview → Approved → Filed → Acknowledged
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum SarStatus {
    /// Auto-generated, not yet seen by a compliance officer
    Draft,
    /// Submitted to the review queue
    PendingReview,
    /// Approved by compliance officer, ready to file
    Approved,
    /// Transmitted to NFIU/CBN
    Filed,
    /// Regulator acknowledged receipt
    Acknowledged,
    /// Rejected by compliance officer (will not be filed)
    Rejected,
}

impl std::fmt::Display for SarStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Draft => "Draft",
            Self::PendingReview => "PendingReview",
            Self::Approved => "Approved",
            Self::Filed => "Filed",
            Self::Acknowledged => "Acknowledged",
            Self::Rejected => "Rejected",
        };
        write!(f, "{s}")
    }
}

/// Which regulatory authority this SAR targets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegulatoryAuthority {
    Nfiu,
    Cbn,
}

impl std::fmt::Display for RegulatoryAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nfiu => write!(f, "NFIU"),
            Self::Cbn => write!(f, "CBN"),
        }
    }
}

/// Core SAR record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SarReport {
    pub id: Uuid,
    /// The AML case that triggered this SAR
    pub aml_case_id: Uuid,
    pub transaction_id: Uuid,
    pub wallet_address: String,
    pub status: String,
    pub authority: String,
    /// Aggregated activity snapshot (JSON)
    pub activity_snapshot: serde_json::Value,
    /// Rendered regulatory payload (XML or JSON string)
    pub rendered_report: Option<String>,
    pub reviewed_by: Option<String>,
    pub review_notes: Option<String>,
    pub filed_at: Option<DateTime<Utc>>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Immutable audit entry for every SAR state transition
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SarAuditEntry {
    pub id: Uuid,
    pub sar_id: Uuid,
    pub actor_id: String,
    pub action: String,
    pub from_status: String,
    pub to_status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Aggregated account activity used to populate the SAR
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySnapshot {
    pub wallet_address: String,
    pub window_hours: u32,
    pub transaction_count: i64,
    pub total_volume: String,
    pub ip_addresses: Vec<String>,
    pub linked_bank_accounts: Vec<String>,
    pub recent_transactions: Vec<TransactionSummary>,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub transaction_id: Uuid,
    pub tx_type: String,
    pub amount: String,
    pub currency: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Request body for compliance officer review actions
#[derive(Debug, Deserialize)]
pub struct ReviewRequest {
    pub officer_id: String,
    pub notes: Option<String>,
    /// Optional edited rendered_report (officer may amend before approval)
    pub amended_report: Option<String>,
}
