//! AML data models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// AML flag severity levels (FATF-aligned)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum AmlFlagLevel {
    /// Level 1 — informational, log only
    Low,
    /// Level 2 — elevated, manual review recommended
    Medium,
    /// Level 3 — critical, instant alert to AML Officer
    Critical,
}

impl std::fmt::Display for AmlFlagLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AmlFlagLevel::Low => write!(f, "LOW"),
            AmlFlagLevel::Medium => write!(f, "MEDIUM"),
            AmlFlagLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Reason a transaction was flagged
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AmlFlag {
    /// Sender or recipient matched a sanctions list entry
    SanctionsHit { list: String, matched_name: String },
    /// Multiple small transactions to same recipient (smurfing)
    SmurfingDetected { tx_count: u32, window_hours: u32, total_amount: String },
    /// Funds on-ramped and immediately off-ramped to high-risk jurisdiction
    RapidFlip { on_ramp_tx_id: Uuid, off_ramp_corridor: String, elapsed_minutes: u32 },
    /// High corridor risk score
    HighCorridorRisk { corridor: String, risk_score: f64, reason: String },
}

/// Lifecycle state of an AML compliance case
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum AmlCaseStatus {
    /// Awaiting compliance officer review
    PendingComplianceReview,
    /// Cleared by compliance officer — transaction may proceed
    Cleared,
    /// Permanently blocked by compliance officer
    PermanentlyBlocked,
}

/// Input to the AML screening pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmlScreeningRequest {
    pub transaction_id: Uuid,
    pub wallet_address: String,
    pub sender_name: String,
    pub sender_id: String,
    pub recipient_name: String,
    pub recipient_id: String,
    pub amount: String,
    pub from_currency: String,
    pub to_currency: String,
    /// ISO 3166-1 alpha-2 origin country
    pub origin_country: String,
    /// ISO 3166-1 alpha-2 destination country
    pub destination_country: String,
    pub created_at: DateTime<Utc>,
}

/// Result of the full AML screening pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmlScreeningResult {
    pub transaction_id: Uuid,
    /// Composite risk score 0.0–1.0
    pub risk_score: f64,
    pub flag_level: Option<AmlFlagLevel>,
    pub flags: Vec<AmlFlag>,
    /// Whether the transaction is cleared to proceed
    pub cleared: bool,
    /// If not cleared, the case ID for compliance review
    pub case_id: Option<Uuid>,
    pub screened_at: DateTime<Utc>,
}

/// Per-corridor risk weight configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorridorRiskWeight {
    pub origin_country: String,
    pub destination_country: String,
    /// 0.0–1.0 weight applied to base risk score
    pub weight: f64,
    /// Human-readable reason (e.g. "FATF Grey List", "Basel AML Index High")
    pub reason: String,
}

/// Detected velocity pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocityPattern {
    pub wallet_address: String,
    pub recipient_id: String,
    pub tx_count: u32,
    pub total_amount: String,
    pub window_hours: u32,
    pub detected_at: DateTime<Utc>,
}
// ---------------------------------------------------------------------------
// CTR (Currency Transaction Report) Models — Issue #390
// ---------------------------------------------------------------------------

/// CTR type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum CtrType {
    Individual,
    Corporate,
}

/// CTR status lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum CtrStatus {
    Draft,
    UnderReview,
    Approved,
    Filed,
    Acknowledged,
    Rejected,
}

/// CTR detection method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum DetectionMethod {
    Automatic,
    Manual,
}

/// Transaction direction for CTR reporting
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum TransactionDirection {
    Debit,
    Credit,
}

/// Currency Transaction Report
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Ctr {
    pub ctr_id: Uuid,
    pub reporting_period: DateTime<Utc>,
    pub ctr_type: CtrType,
    pub subject_kyc_id: Uuid,
    pub subject_full_name: String,
    pub subject_identification: String,
    pub subject_address: String,
    pub total_transaction_amount: rust_decimal::Decimal,
    pub transaction_count: i32,
    pub transaction_references: Vec<String>,
    pub detection_method: DetectionMethod,
    pub status: CtrStatus,
    pub assigned_compliance_officer: Option<Uuid>,
    pub filing_timestamp: Option<DateTime<Utc>>,
    pub regulatory_reference_number: Option<String>,
}

/// CTR aggregation tracking for threshold monitoring
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CtrAggregation {
    pub subject_id: Uuid,
    pub aggregation_window_start: DateTime<Utc>,
    pub aggregation_window_end: DateTime<Utc>,
    pub running_total_amount: rust_decimal::Decimal,
    pub transaction_count: i32,
    pub transaction_amounts: Vec<rust_decimal::Decimal>,
    pub transaction_timestamps: Vec<DateTime<Utc>>,
    pub threshold_breach_flag: bool,
}

/// Individual transaction linked to a CTR
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CtrTransaction {
    pub ctr_id: Uuid,
    pub transaction_id: Uuid,
    pub transaction_timestamp: DateTime<Utc>,
    pub transaction_type: String,
    pub transaction_amount_ngn: rust_decimal::Decimal,
    pub counterparty_details: String,
    pub direction: TransactionDirection,
}

/// CTR filing details and regulatory submission tracking
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CtrFiling {
    pub ctr_id: Uuid,
    pub filing_method: String,
    pub submission_timestamp: DateTime<Utc>,
    pub regulatory_submission_reference: String,
    pub acknowledgement_timestamp: Option<DateTime<Utc>>,
    pub acknowledgement_reference: Option<String>,
    pub rejection_details: Option<String>,
}

/// CTR exemption for subjects that qualify for reporting exemptions
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CtrExemption {
    pub subject_id: Uuid,
    pub exemption_category: String,
    pub exemption_basis: String,
    pub expiry_date: Option<DateTime<Utc>>,
}
