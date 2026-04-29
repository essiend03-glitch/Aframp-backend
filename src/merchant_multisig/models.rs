//! Merchant Multi-Sig & Treasury Controls — Issue #336
//!
//! Data models for:
//!   - Signing policies (M-of-N rules per merchant)
//!   - Signing groups (CFO, Treasury Manager, Auditor, …)
//!   - Proposals (proposed high-value actions awaiting approval)
//!   - Proposal signatures (individual signer decisions)
//!   - Freeze state (emergency 1-of-N account lock)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Signing Policy ────────────────────────────────────────────────────────────

/// The type of merchant action a signing policy governs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Outbound payout to a settlement bank.
    Payout,
    /// Updating API keys.
    ApiKeyUpdate,
    /// Changing tax configuration.
    TaxConfigUpdate,
    /// Catch-all: applies to any action.
    Any,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Payout => "payout",
            Self::ApiKeyUpdate => "api_key_update",
            Self::TaxConfigUpdate => "tax_config_update",
            Self::Any => "any",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "payout" => Self::Payout,
            "api_key_update" => Self::ApiKeyUpdate,
            "tax_config_update" => Self::TaxConfigUpdate,
            _ => Self::Any,
        }
    }
}

/// A configurable M-of-N signing rule for a merchant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningPolicy {
    pub id: Uuid,
    pub merchant_id: String,
    pub policy_name: String,
    pub action_type: ActionType,
    /// Minimum cNGN amount that triggers this policy; `None` = always triggers.
    pub high_value_threshold: Option<rust_decimal::Decimal>,
    /// M — minimum approvals required.
    pub required_signatures: i32,
    /// N — total authorised signers.
    pub total_signers: i32,
    /// Optional: restrict approvals to a specific signing group.
    pub signing_group_id: Option<Uuid>,
    pub is_active: bool,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request body for creating a signing policy.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSigningPolicyRequest {
    pub policy_name: String,
    pub action_type: ActionType,
    pub high_value_threshold: Option<rust_decimal::Decimal>,
    pub required_signatures: i32,
    pub total_signers: i32,
    pub signing_group_id: Option<Uuid>,
}

// ── Signing Group ─────────────────────────────────────────────────────────────

/// A named group of authorised signers (e.g. "CFO", "Treasury Manager").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningGroup {
    pub id: Uuid,
    pub merchant_id: String,
    pub group_name: String,
    pub description: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

/// Request body for creating a signing group.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSigningGroupRequest {
    pub group_name: String,
    pub description: Option<String>,
}

/// A member of a signing group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningGroupMember {
    pub id: Uuid,
    pub group_id: Uuid,
    pub signer_id: String,
    pub signer_name: String,
    pub signer_role: String,
    pub is_active: bool,
    pub added_by: String,
    pub added_at: DateTime<Utc>,
}

/// Request body for adding a member to a signing group.
#[derive(Debug, Clone, Deserialize)]
pub struct AddGroupMemberRequest {
    pub signer_id: String,
    pub signer_name: String,
    pub signer_role: String,
}

// ── Proposal ──────────────────────────────────────────────────────────────────

/// Status of a multi-sig proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    /// Awaiting signatures.
    Pending,
    /// M-of-N threshold met; ready to execute.
    Approved,
    /// Rejected by a signer.
    Rejected,
    /// Expired without reaching threshold.
    Expired,
    /// Action has been executed.
    Executed,
}

impl ProposalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::Executed => "executed",
        }
    }
}

/// A proposed high-value action awaiting multi-sig approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: Uuid,
    pub merchant_id: String,
    pub policy_id: Uuid,
    pub action_type: ActionType,
    /// Full serialised action payload (payout details, new API key metadata, etc.).
    pub action_payload: serde_json::Value,
    /// Amount in cNGN (for payout proposals).
    pub amount: Option<rust_decimal::Decimal>,
    pub status: ProposalStatus,
    pub proposed_by: String,
    pub proposed_by_name: String,
    pub expires_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Signatures collected so far (populated on read).
    #[serde(default)]
    pub signatures: Vec<ProposalSignature>,
}

/// Request body for creating a proposal.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateProposalRequest {
    pub action_type: ActionType,
    pub action_payload: serde_json::Value,
    pub amount: Option<rust_decimal::Decimal>,
    pub proposed_by_name: String,
}

// ── Proposal Signature ────────────────────────────────────────────────────────

/// A signer's decision on a proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerDecision {
    Approved,
    Rejected,
}

impl SignerDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }
}

/// An individual signer's approval or rejection of a proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSignature {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub signer_id: String,
    pub signer_name: String,
    pub signer_role: String,
    pub decision: SignerDecision,
    pub comment: Option<String>,
    pub signed_at: DateTime<Utc>,
}

/// Request body for signing a proposal.
#[derive(Debug, Clone, Deserialize)]
pub struct SignProposalRequest {
    pub signer_name: String,
    pub signer_role: String,
    pub decision: SignerDecision,
    pub comment: Option<String>,
}

// ── Freeze State ──────────────────────────────────────────────────────────────

/// Emergency freeze state for a merchant account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeState {
    pub id: Uuid,
    pub merchant_id: String,
    pub is_frozen: bool,
    pub frozen_by: Option<String>,
    pub frozen_by_name: Option<String>,
    pub freeze_reason: Option<String>,
    pub frozen_at: Option<DateTime<Utc>>,
    pub unfrozen_by: Option<String>,
    pub unfrozen_at: Option<DateTime<Utc>>,
    pub unfreeze_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Request body for triggering an emergency freeze.
#[derive(Debug, Clone, Deserialize)]
pub struct FreezeRequest {
    pub officer_name: String,
    pub reason: String,
}

/// Request body for lifting an emergency freeze.
#[derive(Debug, Clone, Deserialize)]
pub struct UnfreezeRequest {
    pub officer_name: String,
    pub reason: String,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum MultisigError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("proposal not found: {0}")]
    ProposalNotFound(Uuid),
    #[error("signing policy not found for merchant {0}")]
    PolicyNotFound(String),
    #[error("signer {0} has already signed this proposal")]
    DuplicateSignature(String),
    #[error("proposal is not in pending state (current: {0})")]
    ProposalNotPending(String),
    #[error("merchant account is frozen — all outgoing actions are blocked")]
    AccountFrozen,
    #[error("amount {amount} does not meet the high-value threshold {threshold} for policy {policy}")]
    BelowThreshold {
        amount: String,
        threshold: String,
        policy: String,
    },
    #[error("no active signing policy found for action '{0}' on merchant '{1}'")]
    NoPolicyApplicable(String, String),
    #[error("signing group not found: {0}")]
    GroupNotFound(Uuid),
    #[error("signer {0} is not a member of the required signing group")]
    NotGroupMember(String),
}

pub type MultisigResult<T> = Result<T, MultisigError>;
