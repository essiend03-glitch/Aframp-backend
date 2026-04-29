//! Merchant Multi-Sig service — policy enforcement, approval pipeline, freeze.

use crate::audit::models::{AuditActorType, AuditEventCategory, AuditOutcome, PendingAuditEntry};
use crate::audit::writer::AuditWriter;
use crate::merchant_multisig::models::*;
use bigdecimal::BigDecimal;
use chrono::Utc;
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub struct MerchantMultisigService {
    pool: PgPool,
    audit: Option<Arc<AuditWriter>>,
}

impl MerchantMultisigService {
    pub fn new(pool: PgPool, audit: Option<Arc<AuditWriter>>) -> Self {
        Self { pool, audit }
    }

    // ── Freeze ────────────────────────────────────────────────────────────────

    /// Emergency freeze: 1-of-N (CEO / Security Officer) locks all outgoing funds.
    pub async fn freeze(&self, merchant_id: &str, officer_id: &str, req: FreezeRequest) -> MultisigResult<FreezeState> {
        sqlx::query(
            r#"INSERT INTO merchant_freeze_state
               (merchant_id, is_frozen, frozen_by, frozen_by_name, freeze_reason, frozen_at, updated_at)
               VALUES ($1, true, $2, $3, $4, now(), now())
               ON CONFLICT (merchant_id) DO UPDATE
               SET is_frozen=true, frozen_by=$2, frozen_by_name=$3,
                   freeze_reason=$4, frozen_at=now(), updated_at=now()"#,
        )
        .bind(merchant_id).bind(officer_id).bind(&req.officer_name).bind(&req.reason)
        .execute(&self.pool).await?;

        warn!(merchant_id, officer_id, "🔒 Merchant account FROZEN");
        self.audit_event(
            "merchant.freeze", AuditEventCategory::Security, AuditOutcome::Success,
            officer_id, merchant_id, None,
        ).await;
        self.get_freeze_state(merchant_id).await
    }

    /// Lift an emergency freeze.
    pub async fn unfreeze(&self, merchant_id: &str, officer_id: &str, req: UnfreezeRequest) -> MultisigResult<FreezeState> {
        sqlx::query(
            r#"INSERT INTO merchant_freeze_state
               (merchant_id, is_frozen, unfrozen_by, unfrozen_at, unfreeze_reason, updated_at)
               VALUES ($1, false, $2, now(), $3, now())
               ON CONFLICT (merchant_id) DO UPDATE
               SET is_frozen=false, unfrozen_by=$2, unfrozen_at=now(),
                   unfreeze_reason=$3, updated_at=now()"#,
        )
        .bind(merchant_id).bind(officer_id).bind(&req.reason)
        .execute(&self.pool).await?;

        info!(merchant_id, officer_id, "🔓 Merchant account UNFROZEN");
        self.audit_event(
            "merchant.unfreeze", AuditEventCategory::Security, AuditOutcome::Success,
            officer_id, merchant_id, None,
        ).await;
        self.get_freeze_state(merchant_id).await
    }

    pub async fn get_freeze_state(&self, merchant_id: &str) -> MultisigResult<FreezeState> {
        let row = sqlx::query_as::<_, FreezeStateRow>(
            "SELECT * FROM merchant_freeze_state WHERE merchant_id = $1",
        )
        .bind(merchant_id)
        .fetch_optional(&self.pool).await?;

        Ok(match row {
            Some(r) => r.into(),
            None => FreezeState {
                id: Uuid::new_v4(),
                merchant_id: merchant_id.to_string(),
                is_frozen: false,
                frozen_by: None, frozen_by_name: None, freeze_reason: None, frozen_at: None,
                unfrozen_by: None, unfrozen_at: None, unfreeze_reason: None,
                updated_at: Utc::now(),
            },
        })
    }

    fn assert_not_frozen_sync(is_frozen: bool) -> MultisigResult<()> {
        if is_frozen { Err(MultisigError::AccountFrozen) } else { Ok(()) }
    }

    // ── Signing Policies ──────────────────────────────────────────────────────

    pub async fn create_policy(&self, merchant_id: &str, creator_id: &str, req: CreateSigningPolicyRequest) -> MultisigResult<SigningPolicy> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO merchant_signing_policies
               (id, merchant_id, policy_name, action_type, high_value_threshold,
                required_signatures, total_signers, signing_group_id, created_by)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
        )
        .bind(id).bind(merchant_id).bind(&req.policy_name)
        .bind(req.action_type.as_str())
        .bind(req.high_value_threshold.map(|d| d.to_string()))
        .bind(req.required_signatures).bind(req.total_signers)
        .bind(req.signing_group_id).bind(creator_id)
        .execute(&self.pool).await?;

        self.audit_event("merchant.policy.created", AuditEventCategory::Configuration,
            AuditOutcome::Success, creator_id, merchant_id, Some(id.to_string())).await;
        self.get_policy(id).await
    }

    pub async fn list_policies(&self, merchant_id: &str) -> MultisigResult<Vec<SigningPolicy>> {
        let rows = sqlx::query_as::<_, PolicyRow>(
            "SELECT * FROM merchant_signing_policies WHERE merchant_id=$1 AND is_active=true ORDER BY created_at DESC",
        )
        .bind(merchant_id).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_policy(&self, id: Uuid) -> MultisigResult<SigningPolicy> {
        sqlx::query_as::<_, PolicyRow>("SELECT * FROM merchant_signing_policies WHERE id=$1")
            .bind(id).fetch_optional(&self.pool).await?
            .map(Into::into)
            .ok_or(MultisigError::PolicyNotFound(id.to_string()))
    }

    /// Find the most-specific active policy for a given action + amount.
    async fn find_applicable_policy(&self, merchant_id: &str, action_type: &ActionType, amount: Option<&BigDecimal>) -> MultisigResult<SigningPolicy> {
        let rows = sqlx::query_as::<_, PolicyRow>(
            r#"SELECT * FROM merchant_signing_policies
               WHERE merchant_id=$1 AND is_active=true
               AND (action_type=$2 OR action_type='any')
               ORDER BY high_value_threshold DESC NULLS LAST, created_at DESC"#,
        )
        .bind(merchant_id).bind(action_type.as_str())
        .fetch_all(&self.pool).await?;

        for row in rows {
            let policy: SigningPolicy = row.into();
            if let Some(threshold) = &policy.high_value_threshold {
                if let Some(amt) = amount {
                    let t = BigDecimal::from_str(&threshold.to_string()).unwrap_or_default();
                    if amt >= &t { return Ok(policy); }
                }
            } else {
                return Ok(policy);
            }
        }
        Err(MultisigError::NoPolicyApplicable(action_type.as_str().to_string(), merchant_id.to_string()))
    }

    // ── Signing Groups ────────────────────────────────────────────────────────

    pub async fn create_group(&self, merchant_id: &str, creator_id: &str, req: CreateSigningGroupRequest) -> MultisigResult<SigningGroup> {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO merchant_signing_groups (id, merchant_id, group_name, description, created_by) VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(id).bind(merchant_id).bind(&req.group_name).bind(&req.description).bind(creator_id)
        .execute(&self.pool).await?;
        self.get_group(id).await
    }

    pub async fn add_group_member(&self, group_id: Uuid, adder_id: &str, req: AddGroupMemberRequest) -> MultisigResult<SigningGroupMember> {
        let _ = self.get_group(group_id).await?;
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO merchant_signing_group_members (id, group_id, signer_id, signer_name, signer_role, added_by) VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(id).bind(group_id).bind(&req.signer_id).bind(&req.signer_name).bind(&req.signer_role).bind(adder_id)
        .execute(&self.pool).await?;
        Ok(SigningGroupMember { id, group_id, signer_id: req.signer_id, signer_name: req.signer_name, signer_role: req.signer_role, is_active: true, added_by: adder_id.to_string(), added_at: Utc::now() })
    }

    async fn get_group(&self, id: Uuid) -> MultisigResult<SigningGroup> {
        sqlx::query_as::<_, GroupRow>("SELECT * FROM merchant_signing_groups WHERE id=$1")
            .bind(id).fetch_optional(&self.pool).await?
            .map(Into::into)
            .ok_or(MultisigError::GroupNotFound(id))
    }

    async fn is_group_member(&self, group_id: Uuid, signer_id: &str) -> MultisigResult<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM merchant_signing_group_members WHERE group_id=$1 AND signer_id=$2 AND is_active=true",
        )
        .bind(group_id).bind(signer_id).fetch_one(&self.pool).await?;
        Ok(count > 0)
    }

    // ── Proposals ─────────────────────────────────────────────────────────────

    /// Create a proposal. Automatically selects the applicable signing policy.
    /// Blocks if the merchant account is frozen.
    pub async fn create_proposal(&self, merchant_id: &str, proposer_id: &str, req: CreateProposalRequest) -> MultisigResult<Proposal> {
        // 1. Freeze check
        let freeze = self.get_freeze_state(merchant_id).await?;
        Self::assert_not_frozen_sync(freeze.is_frozen)?;

        // 2. Find applicable policy
        let amount_bd = req.amount.as_ref().map(|d| BigDecimal::from_str(&d.to_string()).unwrap_or_default());
        let policy = self.find_applicable_policy(merchant_id, &req.action_type, amount_bd.as_ref()).await?;

        // 3. Persist proposal
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO merchant_proposals
               (id, merchant_id, policy_id, action_type, action_payload, amount,
                proposed_by, proposed_by_name)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
        )
        .bind(id).bind(merchant_id).bind(policy.id)
        .bind(req.action_type.as_str()).bind(&req.action_payload)
        .bind(req.amount.map(|d| d.to_string()))
        .bind(proposer_id).bind(&req.proposed_by_name)
        .execute(&self.pool).await?;

        info!(proposal_id=%id, merchant_id, policy_id=%policy.id, "Merchant proposal created");
        self.audit_event("merchant.proposal.created", AuditEventCategory::FinancialTransaction,
            AuditOutcome::Success, proposer_id, merchant_id, Some(id.to_string())).await;

        self.get_proposal(id).await
    }

    pub async fn get_proposal(&self, id: Uuid) -> MultisigResult<Proposal> {
        let row = sqlx::query_as::<_, ProposalRow>("SELECT * FROM merchant_proposals WHERE id=$1")
            .bind(id).fetch_optional(&self.pool).await?
            .ok_or(MultisigError::ProposalNotFound(id))?;

        let sigs = sqlx::query_as::<_, SignatureRow>(
            "SELECT * FROM merchant_proposal_signatures WHERE proposal_id=$1 ORDER BY signed_at",
        )
        .bind(id).fetch_all(&self.pool).await?;

        let mut proposal: Proposal = row.into();
        proposal.signatures = sigs.into_iter().map(Into::into).collect();
        Ok(proposal)
    }

    pub async fn list_proposals(&self, merchant_id: &str, status: Option<&str>) -> MultisigResult<Vec<Proposal>> {
        let rows = match status {
            Some(s) => sqlx::query_as::<_, ProposalRow>(
                "SELECT * FROM merchant_proposals WHERE merchant_id=$1 AND status=$2 ORDER BY created_at DESC",
            ).bind(merchant_id).bind(s).fetch_all(&self.pool).await?,
            None => sqlx::query_as::<_, ProposalRow>(
                "SELECT * FROM merchant_proposals WHERE merchant_id=$1 ORDER BY created_at DESC",
            ).bind(merchant_id).fetch_all(&self.pool).await?,
        };

        let mut proposals = Vec::new();
        for row in rows {
            let id = row.id;
            let mut p: Proposal = row.into();
            let sigs = sqlx::query_as::<_, SignatureRow>(
                "SELECT * FROM merchant_proposal_signatures WHERE proposal_id=$1 ORDER BY signed_at",
            ).bind(id).fetch_all(&self.pool).await?;
            p.signatures = sigs.into_iter().map(Into::into).collect();
            proposals.push(p);
        }
        Ok(proposals)
    }

    /// Sign (approve or reject) a proposal. Advances status when M threshold is met.
    pub async fn sign_proposal(&self, proposal_id: Uuid, signer_id: &str, req: SignProposalRequest) -> MultisigResult<Proposal> {
        let proposal = self.get_proposal(proposal_id).await?;

        if proposal.status != ProposalStatus::Pending {
            return Err(MultisigError::ProposalNotPending(proposal.status.as_str().to_string()));
        }
        if proposal.signatures.iter().any(|s| s.signer_id == signer_id) {
            return Err(MultisigError::DuplicateSignature(signer_id.to_string()));
        }

        // If policy has a signing group, verify membership
        let policy = self.get_policy(proposal.policy_id).await?;
        if let Some(gid) = policy.signing_group_id {
            if !self.is_group_member(gid, signer_id).await? {
                return Err(MultisigError::NotGroupMember(signer_id.to_string()));
            }
        }

        // Record signature
        sqlx::query(
            "INSERT INTO merchant_proposal_signatures (proposal_id, signer_id, signer_name, signer_role, decision, comment) VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(proposal_id).bind(signer_id).bind(&req.signer_name)
        .bind(&req.signer_role).bind(req.decision.as_str()).bind(&req.comment)
        .execute(&self.pool).await?;

        // Count approvals
        let approval_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM merchant_proposal_signatures WHERE proposal_id=$1 AND decision='approved'",
        ).bind(proposal_id).fetch_one(&self.pool).await?;

        let new_status = if req.decision == SignerDecision::Rejected {
            "rejected"
        } else if approval_count >= policy.required_signatures as i64 {
            "approved"
        } else {
            "pending"
        };

        let now = Utc::now();
        match new_status {
            "approved" => sqlx::query("UPDATE merchant_proposals SET status='approved', approved_at=$1 WHERE id=$2")
                .bind(now).bind(proposal_id).execute(&self.pool).await?,
            "rejected" => sqlx::query("UPDATE merchant_proposals SET status='rejected', rejected_at=$1, rejection_reason=$2 WHERE id=$3")
                .bind(now).bind(req.comment.as_deref().unwrap_or("Rejected by signer")).bind(proposal_id).execute(&self.pool).await?,
            _ => sqlx::query("SELECT 1").execute(&self.pool).await?,
        };

        info!(proposal_id=%proposal_id, signer_id, decision=%req.decision.as_str(), approvals=approval_count, "Proposal signed");
        self.audit_event(
            &format!("merchant.proposal.{}", req.decision.as_str()),
            AuditEventCategory::FinancialTransaction, AuditOutcome::Success,
            signer_id, &proposal.merchant_id, Some(proposal_id.to_string()),
        ).await;

        self.get_proposal(proposal_id).await
    }

    /// Mark an approved proposal as executed.
    pub async fn execute_proposal(&self, proposal_id: Uuid, executor_id: &str) -> MultisigResult<Proposal> {
        let proposal = self.get_proposal(proposal_id).await?;
        if proposal.status != ProposalStatus::Approved {
            return Err(MultisigError::ProposalNotPending(proposal.status.as_str().to_string()));
        }
        // Freeze check before execution
        let freeze = self.get_freeze_state(&proposal.merchant_id).await?;
        Self::assert_not_frozen_sync(freeze.is_frozen)?;

        sqlx::query("UPDATE merchant_proposals SET status='executed', executed_at=now() WHERE id=$1")
            .bind(proposal_id).execute(&self.pool).await?;

        self.audit_event("merchant.proposal.executed", AuditEventCategory::FinancialTransaction,
            AuditOutcome::Success, executor_id, &proposal.merchant_id, Some(proposal_id.to_string())).await;
        self.get_proposal(proposal_id).await
    }

    // ── Audit helper ──────────────────────────────────────────────────────────

    async fn audit_event(&self, event_type: &str, category: AuditEventCategory, outcome: AuditOutcome, actor_id: &str, merchant_id: &str, resource_id: Option<String>) {
        let Some(writer) = &self.audit else { return };
        writer.write(PendingAuditEntry {
            event_type: event_type.to_string(),
            event_category: category,
            actor_type: AuditActorType::Consumer,
            actor_id: Some(actor_id.to_string()),
            actor_ip: None,
            actor_consumer_type: Some("merchant".to_string()),
            session_id: None,
            target_resource_type: Some("merchant_proposal".to_string()),
            target_resource_id: resource_id,
            request_method: "INTERNAL".to_string(),
            request_path: format!("/api/v1/merchants/{merchant_id}/multisig"),
            request_body_hash: None,
            response_status: 200,
            response_latency_ms: 0,
            outcome,
            failure_reason: None,
            environment: std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string()),
        }).await;
    }
}

// ── DB row types ──────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct FreezeStateRow {
    id: Uuid,
    merchant_id: String,
    is_frozen: bool,
    frozen_by: Option<String>,
    frozen_by_name: Option<String>,
    freeze_reason: Option<String>,
    frozen_at: Option<chrono::DateTime<Utc>>,
    unfrozen_by: Option<String>,
    unfrozen_at: Option<chrono::DateTime<Utc>>,
    unfreeze_reason: Option<String>,
    updated_at: chrono::DateTime<Utc>,
}

impl From<FreezeStateRow> for FreezeState {
    fn from(r: FreezeStateRow) -> Self {
        Self {
            id: r.id, merchant_id: r.merchant_id, is_frozen: r.is_frozen,
            frozen_by: r.frozen_by, frozen_by_name: r.frozen_by_name,
            freeze_reason: r.freeze_reason, frozen_at: r.frozen_at,
            unfrozen_by: r.unfrozen_by, unfrozen_at: r.unfrozen_at,
            unfreeze_reason: r.unfreeze_reason, updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PolicyRow {
    id: Uuid,
    merchant_id: String,
    policy_name: String,
    action_type: String,
    high_value_threshold: Option<sqlx::types::BigDecimal>,
    required_signatures: i32,
    total_signers: i32,
    signing_group_id: Option<Uuid>,
    is_active: bool,
    created_by: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

impl From<PolicyRow> for SigningPolicy {
    fn from(r: PolicyRow) -> Self {
        Self {
            id: r.id, merchant_id: r.merchant_id, policy_name: r.policy_name,
            action_type: ActionType::from_str(&r.action_type),
            high_value_threshold: r.high_value_threshold.map(|d| {
                rust_decimal::Decimal::from_str(&d.to_string()).unwrap_or_default()
            }),
            required_signatures: r.required_signatures,
            total_signers: r.total_signers,
            signing_group_id: r.signing_group_id,
            is_active: r.is_active,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct GroupRow {
    id: Uuid,
    merchant_id: String,
    group_name: String,
    description: Option<String>,
    created_by: String,
    created_at: chrono::DateTime<Utc>,
}

impl From<GroupRow> for SigningGroup {
    fn from(r: GroupRow) -> Self {
        Self { id: r.id, merchant_id: r.merchant_id, group_name: r.group_name, description: r.description, created_by: r.created_by, created_at: r.created_at }
    }
}

#[derive(sqlx::FromRow)]
struct ProposalRow {
    id: Uuid,
    merchant_id: String,
    policy_id: Uuid,
    action_type: String,
    action_payload: serde_json::Value,
    amount: Option<sqlx::types::BigDecimal>,
    status: String,
    proposed_by: String,
    proposed_by_name: String,
    expires_at: chrono::DateTime<Utc>,
    approved_at: Option<chrono::DateTime<Utc>>,
    executed_at: Option<chrono::DateTime<Utc>>,
    rejected_at: Option<chrono::DateTime<Utc>>,
    rejection_reason: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

impl From<ProposalRow> for Proposal {
    fn from(r: ProposalRow) -> Self {
        let status = match r.status.as_str() {
            "approved" => ProposalStatus::Approved,
            "rejected" => ProposalStatus::Rejected,
            "expired"  => ProposalStatus::Expired,
            "executed" => ProposalStatus::Executed,
            _          => ProposalStatus::Pending,
        };
        Self {
            id: r.id, merchant_id: r.merchant_id, policy_id: r.policy_id,
            action_type: ActionType::from_str(&r.action_type),
            action_payload: r.action_payload,
            amount: r.amount.map(|d| rust_decimal::Decimal::from_str(&d.to_string()).unwrap_or_default()),
            status,
            proposed_by: r.proposed_by, proposed_by_name: r.proposed_by_name,
            expires_at: r.expires_at, approved_at: r.approved_at,
            executed_at: r.executed_at, rejected_at: r.rejected_at,
            rejection_reason: r.rejection_reason, created_at: r.created_at,
            signatures: vec![],
        }
    }
}

#[derive(sqlx::FromRow)]
struct SignatureRow {
    id: Uuid,
    proposal_id: Uuid,
    signer_id: String,
    signer_name: String,
    signer_role: String,
    decision: String,
    comment: Option<String>,
    signed_at: chrono::DateTime<Utc>,
}

impl From<SignatureRow> for ProposalSignature {
    fn from(r: SignatureRow) -> Self {
        Self {
            id: r.id, proposal_id: r.proposal_id,
            signer_id: r.signer_id, signer_name: r.signer_name, signer_role: r.signer_role,
            decision: if r.decision == "approved" { SignerDecision::Approved } else { SignerDecision::Rejected },
            comment: r.comment, signed_at: r.signed_at,
        }
    }
}
