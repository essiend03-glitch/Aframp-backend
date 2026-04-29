use crate::agent_dashboard::{
    repository::AgentDashboardRepository,
    types::{
        AgentOpStatus, AgentRecord, AgentTask, AgentTemplate, ApprovalQueueItem,
        DeployTemplateRequest, InterventionLog,
    },
};
use crate::agent_cfo::types::AgentKeyState;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub struct AgentDashboardService {
    repo: Arc<AgentDashboardRepository>,
    db: PgPool,
}

impl AgentDashboardService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: Arc::new(AgentDashboardRepository::new(db.clone())),
            db,
        }
    }

    pub fn repo(&self) -> &AgentDashboardRepository {
        &self.repo
    }

    // ── Intervention: Pause ───────────────────────────────────────────────────

    /// Immediately pauses an agent: sets op_status = Paused and freezes its
    /// signing keys in the CFO budget policy (revokes Stellar signing authority).
    pub async fn pause_agent(
        &self,
        agent_id: Uuid,
        performed_by: &str,
        reason: Option<&str>,
    ) -> Result<InterventionLog, String> {
        // 1. Freeze signing keys in CFO layer
        sqlx::query!(
            "UPDATE agent_budget_policies \
             SET key_state = 'frozen', updated_at = NOW() \
             WHERE agent_id = $1",
            agent_id,
        )
        .execute(&self.db)
        .await
        .map_err(|e| format!("freeze keys: {e}"))?;

        // 2. Set operational status to Paused
        self.repo
            .set_agent_status(agent_id, AgentOpStatus::Paused)
            .await?;

        warn!(
            agent_id = %agent_id,
            performed_by = %performed_by,
            "⏸️  Agent paused — signing keys frozen"
        );

        self.repo
            .log_intervention(agent_id, "pause", performed_by, reason)
            .await
    }

    // ── Intervention: Resume ──────────────────────────────────────────────────

    pub async fn resume_agent(
        &self,
        agent_id: Uuid,
        performed_by: &str,
        reason: Option<&str>,
    ) -> Result<InterventionLog, String> {
        sqlx::query!(
            "UPDATE agent_budget_policies \
             SET key_state = 'active', updated_at = NOW() \
             WHERE agent_id = $1",
            agent_id,
        )
        .execute(&self.db)
        .await
        .map_err(|e| format!("unfreeze keys: {e}"))?;

        self.repo
            .set_agent_status(agent_id, AgentOpStatus::Idle)
            .await?;

        info!(agent_id = %agent_id, performed_by = %performed_by, "▶️  Agent resumed");

        self.repo
            .log_intervention(agent_id, "resume", performed_by, reason)
            .await
    }

    // ── Intervention: Reset ───────────────────────────────────────────────────

    /// Resets an agent to Idle without touching its signing keys.
    pub async fn reset_agent(
        &self,
        agent_id: Uuid,
        performed_by: &str,
        reason: Option<&str>,
    ) -> Result<InterventionLog, String> {
        self.repo
            .set_agent_status(agent_id, AgentOpStatus::Idle)
            .await?;

        // Cancel any running tasks for this agent
        sqlx::query!(
            "UPDATE agent_tasks SET status = 'failed', completed_at = NOW() \
             WHERE agent_id = $1 AND status IN ('running', 'queued')",
            agent_id,
        )
        .execute(&self.db)
        .await
        .map_err(|e| format!("cancel tasks: {e}"))?;

        info!(agent_id = %agent_id, performed_by = %performed_by, "🔄 Agent reset");

        self.repo
            .log_intervention(agent_id, "reset", performed_by, reason)
            .await
    }

    // ── Intervention: Circuit Breaker ─────────────────────────────────────────

    /// Hard stop: freezes keys + sets status to Error.
    pub async fn circuit_breaker(
        &self,
        agent_id: Uuid,
        performed_by: &str,
        reason: Option<&str>,
    ) -> Result<InterventionLog, String> {
        sqlx::query!(
            "UPDATE agent_budget_policies \
             SET key_state = 'frozen', updated_at = NOW() \
             WHERE agent_id = $1",
            agent_id,
        )
        .execute(&self.db)
        .await
        .map_err(|e| format!("circuit_breaker freeze: {e}"))?;

        self.repo
            .set_agent_status(agent_id, AgentOpStatus::Error)
            .await?;

        warn!(
            agent_id = %agent_id,
            performed_by = %performed_by,
            "🔴 Circuit breaker triggered — agent signing authority revoked"
        );

        self.repo
            .log_intervention(agent_id, "circuit_breaker", performed_by, reason)
            .await
    }

    // ── Approval queue ────────────────────────────────────────────────────────

    pub async fn list_pending_approvals(&self) -> Result<Vec<ApprovalQueueItem>, String> {
        self.repo.list_pending_approvals().await
    }

    /// 1-click approve or reject a queued task.
    pub async fn decide_approval(
        &self,
        item_id: Uuid,
        decision: &str,
        decided_by: &str,
    ) -> Result<ApprovalQueueItem, String> {
        if !matches!(decision, "approved" | "rejected") {
            return Err("decision must be 'approved' or 'rejected'".to_string());
        }
        let item = self.repo.decide_approval(item_id, decision, decided_by).await?;

        // If approved, unblock the task
        if decision == "approved" {
            sqlx::query!(
                "UPDATE agent_tasks SET status = 'queued' \
                 WHERE id = $1 AND status = 'awaiting_approval'",
                item.task_id,
            )
            .execute(&self.db)
            .await
            .map_err(|e| format!("unblock task: {e}"))?;
        } else {
            sqlx::query!(
                "UPDATE agent_tasks SET status = 'failed', completed_at = NOW() \
                 WHERE id = $1 AND status = 'awaiting_approval'",
                item.task_id,
            )
            .execute(&self.db)
            .await
            .map_err(|e| format!("reject task: {e}"))?;
        }

        self.repo
            .log_intervention(
                item.agent_id,
                "authorize",
                decided_by,
                Some(&format!("approval_item={item_id} decision={decision}")),
            )
            .await?;

        Ok(item)
    }

    // ── Templates ─────────────────────────────────────────────────────────────

    pub async fn create_template(
        &self,
        name: &str,
        instructions: &str,
        created_by: &str,
    ) -> Result<AgentTemplate, String> {
        self.repo.create_template(name, instructions, created_by).await
    }

    pub async fn list_templates(&self) -> Result<Vec<AgentTemplate>, String> {
        self.repo.list_templates().await
    }

    pub async fn deploy_template(
        &self,
        req: DeployTemplateRequest,
    ) -> Result<serde_json::Value, String> {
        // Validate template exists
        let template = self.repo.get_template(req.template_id).await?;
        let affected = self
            .repo
            .deploy_template(req.template_id, &req.agent_ids)
            .await?;

        info!(
            template_id = %req.template_id,
            template_name = %template.name,
            agents_updated = affected,
            performed_by = %req.performed_by,
            "📦 Agent template deployed"
        );

        Ok(serde_json::json!({
            "template_id": req.template_id,
            "template_name": template.name,
            "agents_updated": affected,
        }))
    }

    // ── Telemetry helpers ─────────────────────────────────────────────────────

    pub async fn get_agent(&self, agent_id: Uuid) -> Result<AgentRecord, String> {
        self.repo.get_agent(agent_id).await
    }

    pub async fn get_task_trace(&self, task_id: Uuid) -> Result<AgentTask, String> {
        self.repo.get_task(task_id).await
    }

    pub async fn audit_export(&self, agent_id: Uuid) -> Result<Vec<serde_json::Value>, String> {
        self.repo.audit_export(agent_id).await
    }
}
