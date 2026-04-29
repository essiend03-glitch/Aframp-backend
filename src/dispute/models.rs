//! Data models for Merchant Dispute Resolution & Clawback Management (Issue #337).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// Lifecycle status of a dispute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "dispute_status", rename_all = "snake_case")]
pub enum DisputeStatus {
    /// Customer has filed; awaiting merchant response (48-hour window).
    Open,
    /// Merchant has responded; awaiting customer review or platform mediation.
    UnderReview,
    /// Platform mediator is actively reviewing the case.
    Mediation,
    /// Resolved in favour of the customer — refund triggered.
    ResolvedCustomer,
    /// Resolved in favour of the merchant — no refund.
    ResolvedMerchant,
    /// Partially resolved — partial refund triggered.
    ResolvedPartial,
    /// Closed without resolution (e.g. customer withdrew).
    Closed,
}

impl DisputeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::UnderReview => "under_review",
            Self::Mediation => "mediation",
            Self::ResolvedCustomer => "resolved_customer",
            Self::ResolvedMerchant => "resolved_merchant",
            Self::ResolvedPartial => "resolved_partial",
            Self::Closed => "closed",
        }
    }

    /// Returns true if the dispute has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::ResolvedCustomer
                | Self::ResolvedMerchant
                | Self::ResolvedPartial
                | Self::Closed
        )
    }
}

/// Category of the dispute claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "dispute_reason", rename_all = "snake_case")]
pub enum DisputeReason {
    ItemNotReceived,
    WrongAmountCharged,
    DamagedGoods,
    UnauthorisedCharge,
    ServiceNotProvided,
    Other,
}

/// Who submitted a piece of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "evidence_submitter", rename_all = "snake_case")]
pub enum EvidenceSubmitter {
    Customer,
    Merchant,
    System,
}

/// Final decision recorded in the audit trail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "dispute_decision", rename_all = "snake_case")]
pub enum DisputeDecision {
    FullRefund,
    PartialRefund,
    NoRefund,
    Withdrawn,
}

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

/// A dispute case filed by a customer against a merchant transaction.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Dispute {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub customer_wallet: String,
    pub merchant_id: Uuid,
    pub reason: DisputeReason,
    pub description: String,
    pub status: DisputeStatus,
    /// Amount originally transacted (in cNGN base units).
    pub transaction_amount: sqlx::types::BigDecimal,
    /// Amount the customer is claiming back (may be partial).
    pub claimed_amount: sqlx::types::BigDecimal,
    /// Amount actually refunded after resolution.
    pub refunded_amount: Option<sqlx::types::BigDecimal>,
    /// Deadline for merchant to respond (filed_at + 48 h).
    pub merchant_response_deadline: DateTime<Utc>,
    /// When the merchant submitted their response.
    pub merchant_responded_at: Option<DateTime<Utc>>,
    /// Settlement proposal offered by the merchant.
    pub settlement_proposal: Option<serde_json::Value>,
    /// Final decision recorded at resolution.
    pub final_decision: Option<DisputeDecision>,
    /// Stellar transaction hash of the clawback/refund operation.
    pub refund_tx_hash: Option<String>,
    /// Whether funds are currently held in provisional escrow.
    pub escrow_active: bool,
    /// Percentage of transaction amount held in escrow (0–100).
    pub escrow_hold_pct: Option<sqlx::types::BigDecimal>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// A piece of evidence attached to a dispute.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DisputeEvidence {
    pub id: Uuid,
    pub dispute_id: Uuid,
    pub submitter: EvidenceSubmitter,
    pub submitter_id: String,
    /// Human-readable label (e.g. "Photo of damaged goods").
    pub label: String,
    /// URL or reference to the stored file/document.
    pub file_url: Option<String>,
    /// Free-text note accompanying the evidence.
    pub notes: Option<String>,
    /// Automatically pulled delivery status from shipping provider.
    pub delivery_status: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Audit trail entry for every state transition on a dispute.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DisputeAuditLog {
    pub id: Uuid,
    pub dispute_id: Uuid,
    pub actor: String,
    pub action: String,
    pub previous_status: Option<DisputeStatus>,
    pub new_status: Option<DisputeStatus>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Customer request to open a new dispute.
#[derive(Debug, Deserialize)]
pub struct OpenDisputeRequest {
    pub transaction_id: Uuid,
    pub reason: DisputeReason,
    pub description: String,
    /// Amount the customer is claiming back (defaults to full transaction amount).
    pub claimed_amount: Option<f64>,
}

/// Merchant response to an open dispute.
#[derive(Debug, Deserialize)]
pub struct MerchantResponseRequest {
    pub notes: String,
    /// Optional settlement proposal.
    pub settlement_proposal: Option<SettlementProposal>,
}

/// A settlement offer from the merchant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementProposal {
    /// "full_refund" | "partial_refund" | "no_refund"
    pub proposal_type: String,
    /// Refund amount for partial proposals (in cNGN).
    pub refund_amount: Option<f64>,
    pub message: Option<String>,
}

/// Platform mediator resolves the dispute.
#[derive(Debug, Deserialize)]
pub struct ResolveDisputeRequest {
    pub decision: DisputeDecision,
    /// Refund amount (required for PartialRefund).
    pub refund_amount: Option<f64>,
    pub notes: String,
}

/// Evidence submission by customer or merchant.
#[derive(Debug, Deserialize)]
pub struct SubmitEvidenceRequest {
    pub label: String,
    pub file_url: Option<String>,
    pub notes: Option<String>,
}

/// Query parameters for listing disputes.
#[derive(Debug, Deserialize)]
pub struct DisputeListQuery {
    pub status: Option<DisputeStatus>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl DisputeListQuery {
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }
    pub fn page_size(&self) -> i64 {
        self.page_size.unwrap_or(20).clamp(1, 100)
    }
    pub fn offset(&self) -> i64 {
        (self.page() - 1) * self.page_size()
    }
}

/// Paginated list of disputes.
#[derive(Debug, Serialize)]
pub struct DisputePage {
    pub disputes: Vec<Dispute>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}
