use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::AppError;
use super::{YieldStrategy, GovernanceApprovalRecord, GovernanceApproval, ApprovalType, GovernanceStatus};

/// DeFi governance committee for managing protocol and strategy approvals
pub struct GovernanceCommittee {
    config: GovernanceConfig,
    committee_members: HashMap<String, CommitteeMember>,
}

impl GovernanceCommittee {
    pub fn new(config: GovernanceConfig) -> Self {
        Self {
            config,
            committee_members: HashMap::new(),
        }
    }

    /// Add a committee member
    pub fn add_member(&mut self, member: CommitteeMember) -> Result<(), AppError> {
        if self.committee_members.contains_key(&member.user_id) {
            return Err(AppError::BadRequest("Committee member already exists".to_string()));
        }

        self.committee_members.insert(member.user_id.clone(), member);
        Ok(())
    }

    /// Remove a committee member
    pub fn remove_member(&mut self, user_id: &str) -> Result<(), AppError> {
        if !self.committee_members.contains_key(user_id) {
            return Err(AppError::BadRequest("Committee member not found".to_string()));
        }

        self.committee_members.remove(user_id);
        Ok(())
    }

    /// Submit a strategy for governance approval
    pub async fn submit_strategy_for_approval(
        &self,
        strategy: &YieldStrategy,
        submitted_by: &str,
    ) -> Result<GovernanceApprovalRecord, AppError> {
        // Validate submitter permissions
        self.validate_submitter_permissions(submitted_by).await?;

        // Create approval record
        let approval_record = GovernanceApprovalRecord {
            record_id: Uuid::new_v4(),
            strategy_id: strategy.strategy_id,
            submitted_by: submitted_by.to_string(),
            submitted_at: Utc::now(),
            required_approvals: self.config.min_approvals_required,
            received_approvals: 0,
            approval_status: GovernanceStatus::Pending,
            approvals: Vec::new(),
            rejection_reason: None,
        };

        tracing::info!(
            strategy_id = %strategy.strategy_id,
            submitted_by = %submitted_by,
            required_approvals = %self.config.min_approvals_required,
            "Strategy submitted for governance approval"
        );

        Ok(approval_record)
    }

    /// Record a committee member's approval
    pub async fn record_approval(
        &self,
        approval_record: &mut GovernanceApprovalRecord,
        committee_member: &str,
        justification: &str,
        approval_type: ApprovalType,
    ) -> Result<(), AppError> {
        // Validate committee member
        self.validate_committee_member(committee_member).await?;

        // Check if member has already voted
        if approval_record.approvals.iter().any(|a| a.committee_member == committee_member) {
            return Err(AppError::BadRequest("Committee member has already voted".to_string()));
        }

        // Check if approval record is still pending
        if !matches!(approval_record.approval_status, GovernanceStatus::Pending) {
            return Err(AppError::BadRequest("Approval record is not pending".to_string()));
        }

        // Record the approval/rejection
        let approval = GovernanceApproval {
            approval_id: Uuid::new_v4(),
            committee_member: committee_member.to_string(),
            approved_at: Utc::now(),
            justification: justification.to_string(),
            approval_type: approval_type.clone(),
        };

        approval_record.approvals.push(approval);

        // Update approval count and status
        approval_record.received_approvals = approval_record.approvals
            .iter()
            .filter(|a| matches!(a.approval_type, ApprovalType::Approve))
            .count();

        // Check if we have enough approvals
        if approval_record.received_approvals >= approval_record.required_approvals {
            approval_record.approval_status = GovernanceStatus::Approved;
            tracing::info!(
                strategy_id = %approval_record.strategy_id,
                approvals_received = %approval_record.received_approvals,
                "Strategy approved by governance committee"
            );
        }

        // Check if rejected (if any rejection and not enough approvals remaining)
        let rejections = approval_record.approvals
            .iter()
            .filter(|a| matches!(a.approval_type, ApprovalType::Reject))
            .count();

        let remaining_members = self.committee_members.len() - approval_record.approvals.len();
        let max_possible_approvals = approval_record.received_approvals + remaining_members;

        if rejections > 0 && max_possible_approvals < approval_record.required_approvals {
            approval_record.approval_status = GovernanceStatus::Rejected;
            approval_record.rejection_reason = Some("Insufficient approvals possible due to rejections".to_string());
            tracing::warn!(
                strategy_id = %approval_record.strategy_id,
                rejections = %rejections,
                "Strategy rejected by governance committee"
            );
        }

        tracing::info!(
            strategy_id = %approval_record.strategy_id,
            committee_member = %committee_member,
            approval_type = ?approval_type,
            justification = %justification,
            approvals_received = %approval_record.received_approvals,
            required_approvals = %approval_record.required_approvals,
            "Committee member vote recorded"
        );

        Ok(())
    }

    /// Check if a strategy can be activated based on governance approval
    pub async fn can_activate_strategy(&self, approval_record: &GovernanceApprovalRecord) -> Result<bool, AppError> {
        Ok(matches!(approval_record.approval_status, GovernanceStatus::Approved))
    }

    /// Get governance statistics
    pub fn get_governance_stats(&self) -> GovernanceStats {
        GovernanceStats {
            total_members: self.committee_members.len(),
            active_members: self.committee_members.values()
                .filter(|m| m.is_active)
                .count(),
            min_approvals_required: self.config.min_approvals_required,
            approval_timeout_hours: self.config.approval_timeout_hours,
        }
    }

    // Private helper methods

    async fn validate_submitter_permissions(&self, submitted_by: &str) -> Result<(), AppError> {
        // Check if submitter is authorized to submit strategies
        if !self.config.authorized_submitters.contains(&submitted_by.to_string()) {
            return Err(AppError::Forbidden("User not authorized to submit strategies".to_string()));
        }

        Ok(())
    }

    async fn validate_committee_member(&self, committee_member: &str) -> Result<(), AppError> {
        let member = self.committee_members.get(committee_member)
            .ok_or_else(|| AppError::Forbidden("User is not a committee member".to_string()))?;

        if !member.is_active {
            return Err(AppError::Forbidden("Committee member is not active".to_string()));
        }

        Ok(())
    }
}

/// Workflow for managing governance approvals
pub struct ApprovalWorkflow {
    committee: GovernanceCommittee,
}

impl ApprovalWorkflow {
    pub fn new(committee: GovernanceCommittee) -> Self {
        Self { committee }
    }

    /// Process a strategy through the complete approval workflow
    pub async fn process_strategy_approval(
        &self,
        strategy: &YieldStrategy,
        submitted_by: &str,
    ) -> Result<WorkflowResult, AppError> {
        // Submit for approval
        let mut approval_record = self.committee.submit_strategy_for_approval(strategy, submitted_by).await?;

        // Check if we have enough committee members for approval
        let stats = self.committee.get_governance_stats();
        if stats.active_members < stats.min_approvals_required {
            return Err(AppError::InternalServerError(
                "Insufficient active committee members for approval".to_string()
            ));
        }

        Ok(WorkflowResult {
            approval_record,
            status: WorkflowStatus::PendingApproval,
            next_steps: self.get_next_approval_steps(&approval_record),
        })
    }

    /// Get next steps for approval workflow
    fn get_next_approval_steps(&self, approval_record: &GovernanceApprovalRecord) -> Vec<String> {
        let mut steps = Vec::new();

        let remaining_approvals = approval_record.required_approvals - approval_record.received_approvals;
        if remaining_approvals > 0 {
            steps.push(format!("Need {} more committee member approvals", remaining_approvals));
        }

        steps.push("Wait for committee member votes".to_string());
        steps.push("Monitor approval progress".to_string());

        if approval_record.received_approvals >= approval_record.required_approvals {
            steps.push("Strategy can be activated".to_string());
        }

        steps
    }
}

/// Committee member configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitteeMember {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub role: CommitteeRole,
    pub is_active: bool,
    pub joined_at: DateTime<Utc>,
    pub expertise_areas: Vec<String>,
}

/// Committee role
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitteeRole {
    Chair,
    Member,
    Observer,
}

/// Governance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    pub min_approvals_required: usize,
    pub approval_timeout_hours: i64,
    pub authorized_submitters: Vec<String>,
    pub emergency_approval_required: bool,
    pub quorum_percentage: f64,
}

/// Governance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStats {
    pub total_members: usize,
    pub active_members: usize,
    pub min_approvals_required: usize,
    pub approval_timeout_hours: i64,
}

/// Workflow result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub approval_record: GovernanceApprovalRecord,
    pub status: WorkflowStatus,
    pub next_steps: Vec<String>,
}

/// Workflow status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    PendingApproval,
    Approved,
    Rejected,
    Expired,
}

/// Protocol approval record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProtocolApprovalRecord {
    pub approval_id: Uuid,
    pub protocol_id: String,
    pub submitted_by: String,
    pub submitted_at: DateTime<Utc>,
    pub required_approvals: usize,
    pub received_approvals: usize,
    pub approval_status: GovernanceStatus,
    pub approvals: Vec<GovernanceApproval>,
    pub rejection_reason: Option<String>,
    pub governance_review_summary: Option<String>,
    pub risk_assessment: Option<String>,
    pub compliance_check: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Strategy change approval record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StrategyChangeApprovalRecord {
    pub change_id: Uuid,
    pub strategy_id: Uuid,
    pub change_type: StrategyChangeType,
    pub change_description: String,
    pub submitted_by: String,
    pub submitted_at: DateTime<Utc>,
    pub required_approvals: usize,
    pub received_approvals: usize,
    pub approval_status: GovernanceStatus,
    pub approvals: Vec<GovernanceApproval>,
    pub rejection_reason: Option<String>,
    pub impact_assessment: Option<String>,
    pub rollback_plan: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Strategy change type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "strategy_change_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum StrategyChangeType {
    AllocationChange,
    RiskParameterChange,
    RebalancingFrequencyChange,
    ProtocolAddition,
    ProtocolRemoval,
    YieldRateTargetChange,
    EmergencySuspension,
}

/// Governance audit log
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GovernanceAuditLog {
    pub log_id: Uuid,
    pub entity_type: GovernanceEntityType,
    pub entity_id: String,
    pub action: GovernanceAction,
    pub performed_by: String,
    pub performed_at: DateTime<Utc>,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Governance entity type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "governance_entity_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum GovernanceEntityType {
    Protocol,
    Strategy,
    CommitteeMember,
    ApprovalRecord,
}

/// Governance action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "governance_action", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum GovernanceAction {
    Submit,
    Approve,
    Reject,
    Activate,
    Suspend,
    Modify,
    Delete,
}
