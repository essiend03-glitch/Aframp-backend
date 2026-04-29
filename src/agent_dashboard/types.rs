use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Operational state of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "agent_op_status", rename_all = "snake_case")]
pub enum AgentOpStatus {
    Idle,
    Working,
    Negotiating,
    Paused,
    Error,
}

/// A registered agent with its current telemetry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentRecord {
    pub id: Uuid,
    pub name: String,
    pub template_id: Option<Uuid>,
    pub op_status: AgentOpStatus,
    /// cNGN burned in the current rolling minute.
    pub burn_rate_cngn: String,
    /// cNGN spent today.
    pub spent_today_cngn: String,
    /// cNGN daily limit (denormalised from budget policy for fast reads).
    pub daily_limit_cngn: String,
    /// Current wallet balance.
    pub wallet_balance_cngn: String,
    /// Total tasks completed lifetime.
    pub tasks_completed: i64,
    /// Total tasks failed lifetime.
    pub tasks_failed: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A task currently being executed (or queued) by an agent.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentTask {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub description: String,
    /// "queued" | "running" | "completed" | "failed" | "awaiting_approval"
    pub status: String,
    pub projected_cost_cngn: String,
    pub actual_cost_cngn: Option<String>,
    /// Reasoning trace — agent's internal thought log (JSONB array of steps).
    pub reasoning_trace: serde_json::Value,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// An item in the Human Approval Queue — tasks that exceeded budget or risk limits.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApprovalQueueItem {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub task_id: Uuid,
    pub reason: String,
    pub projected_cost_cngn: String,
    /// "pending" | "approved" | "rejected"
    pub decision: String,
    pub decided_by: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// A manual intervention record (pause / reset / circuit-breaker).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InterventionLog {
    pub id: Uuid,
    pub agent_id: Uuid,
    /// "pause" | "reset" | "circuit_breaker" | "authorize" | "resume"
    pub action: String,
    pub performed_by: String,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// An agent template — instructions applied to a fleet of agents at once.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentTemplate {
    pub id: Uuid,
    pub name: String,
    pub instructions: String,
    pub version: i32,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct InterventionRequest {
    /// "pause" | "reset" | "circuit_breaker" | "resume"
    pub action: String,
    pub performed_by: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalDecisionRequest {
    /// "approved" | "rejected"
    pub decision: String,
    pub decided_by: String,
}

#[derive(Debug, Deserialize)]
pub struct DeployTemplateRequest {
    pub template_id: Uuid,
    /// Agent IDs to update; empty = all agents using the old template version.
    pub agent_ids: Vec<Uuid>,
    pub performed_by: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub instructions: String,
    pub created_by: String,
}

/// Query params for listing agents.
#[derive(Debug, Deserialize)]
pub struct AgentListQuery {
    pub status: Option<AgentOpStatus>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl AgentListQuery {
    pub fn page(&self) -> i64 { self.page.unwrap_or(1).max(1) }
    pub fn page_size(&self) -> i64 { self.page_size.unwrap_or(50).clamp(1, 200) }
    pub fn offset(&self) -> i64 { (self.page() - 1) * self.page_size() }
}

/// Audit-ready export row.
#[derive(Debug, Serialize)]
pub struct AuditExportRow {
    pub timestamp: DateTime<Utc>,
    pub agent_id: Uuid,
    pub event_kind: String,
    pub actor: String,
    pub detail: serde_json::Value,
}
