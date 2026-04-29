use crate::agent_dashboard::types::{
    AgentListQuery, AgentOpStatus, AgentRecord, AgentTask, AgentTemplate, ApprovalQueueItem,
    InterventionLog,
};
use sqlx::PgPool;
use uuid::Uuid;

pub struct AgentDashboardRepository {
    db: PgPool,
}

impl AgentDashboardRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    // ── Agent telemetry ───────────────────────────────────────────────────────

    pub async fn list_agents(&self, q: &AgentListQuery) -> Result<Vec<AgentRecord>, String> {
        sqlx::query_as!(
            AgentRecord,
            r#"
            SELECT id, name, template_id,
                   op_status AS "op_status: AgentOpStatus",
                   burn_rate_cngn, spent_today_cngn, daily_limit_cngn,
                   wallet_balance_cngn, tasks_completed, tasks_failed,
                   created_at, updated_at
            FROM agent_registry
            WHERE ($1::agent_op_status IS NULL OR op_status = $1)
            ORDER BY updated_at DESC
            LIMIT $2 OFFSET $3
            "#,
            q.status as Option<AgentOpStatus>,
            q.page_size(),
            q.offset(),
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_agents: {e}"))
    }

    pub async fn get_agent(&self, agent_id: Uuid) -> Result<AgentRecord, String> {
        sqlx::query_as!(
            AgentRecord,
            r#"
            SELECT id, name, template_id,
                   op_status AS "op_status: AgentOpStatus",
                   burn_rate_cngn, spent_today_cngn, daily_limit_cngn,
                   wallet_balance_cngn, tasks_completed, tasks_failed,
                   created_at, updated_at
            FROM agent_registry WHERE id = $1
            "#,
            agent_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("get_agent: {e}"))
    }

    pub async fn set_agent_status(
        &self,
        agent_id: Uuid,
        status: AgentOpStatus,
    ) -> Result<(), String> {
        sqlx::query!(
            "UPDATE agent_registry SET op_status = $2, updated_at = NOW() WHERE id = $1",
            agent_id,
            status as AgentOpStatus,
        )
        .execute(&self.db)
        .await
        .map(|_| ())
        .map_err(|e| format!("set_agent_status: {e}"))
    }

    // ── Tasks ─────────────────────────────────────────────────────────────────

    pub async fn list_tasks(&self, agent_id: Uuid) -> Result<Vec<AgentTask>, String> {
        sqlx::query_as!(
            AgentTask,
            r#"
            SELECT id, agent_id, description, status, projected_cost_cngn,
                   actual_cost_cngn, reasoning_trace, started_at, completed_at, created_at
            FROM agent_tasks WHERE agent_id = $1
            ORDER BY created_at DESC LIMIT 100
            "#,
            agent_id,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_tasks: {e}"))
    }

    pub async fn get_task(&self, task_id: Uuid) -> Result<AgentTask, String> {
        sqlx::query_as!(
            AgentTask,
            r#"
            SELECT id, agent_id, description, status, projected_cost_cngn,
                   actual_cost_cngn, reasoning_trace, started_at, completed_at, created_at
            FROM agent_tasks WHERE id = $1
            "#,
            task_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("get_task: {e}"))
    }

    // ── Approval queue ────────────────────────────────────────────────────────

    pub async fn list_pending_approvals(&self) -> Result<Vec<ApprovalQueueItem>, String> {
        sqlx::query_as!(
            ApprovalQueueItem,
            r#"
            SELECT id, agent_id, task_id, reason, projected_cost_cngn,
                   decision, decided_by, decided_at, created_at
            FROM agent_approval_queue
            WHERE decision = 'pending'
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_pending_approvals: {e}"))
    }

    pub async fn decide_approval(
        &self,
        item_id: Uuid,
        decision: &str,
        decided_by: &str,
    ) -> Result<ApprovalQueueItem, String> {
        sqlx::query_as!(
            ApprovalQueueItem,
            r#"
            UPDATE agent_approval_queue
            SET decision = $2, decided_by = $3, decided_at = NOW()
            WHERE id = $1
            RETURNING id, agent_id, task_id, reason, projected_cost_cngn,
                      decision, decided_by, decided_at, created_at
            "#,
            item_id,
            decision,
            decided_by,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("decide_approval: {e}"))
    }

    // ── Intervention log ──────────────────────────────────────────────────────

    pub async fn log_intervention(
        &self,
        agent_id: Uuid,
        action: &str,
        performed_by: &str,
        reason: Option<&str>,
    ) -> Result<InterventionLog, String> {
        sqlx::query_as!(
            InterventionLog,
            r#"
            INSERT INTO agent_intervention_logs
                (id, agent_id, action, performed_by, reason, created_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())
            RETURNING id, agent_id, action, performed_by, reason, created_at
            "#,
            agent_id,
            action,
            performed_by,
            reason,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("log_intervention: {e}"))
    }

    pub async fn list_interventions(
        &self,
        agent_id: Uuid,
    ) -> Result<Vec<InterventionLog>, String> {
        sqlx::query_as!(
            InterventionLog,
            r#"
            SELECT id, agent_id, action, performed_by, reason, created_at
            FROM agent_intervention_logs
            WHERE agent_id = $1
            ORDER BY created_at DESC LIMIT 200
            "#,
            agent_id,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_interventions: {e}"))
    }

    // ── Templates ─────────────────────────────────────────────────────────────

    pub async fn create_template(
        &self,
        name: &str,
        instructions: &str,
        created_by: &str,
    ) -> Result<AgentTemplate, String> {
        sqlx::query_as!(
            AgentTemplate,
            r#"
            INSERT INTO agent_templates (id, name, instructions, version, created_by, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, $2, 1, $3, NOW(), NOW())
            RETURNING id, name, instructions, version, created_by, created_at, updated_at
            "#,
            name,
            instructions,
            created_by,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("create_template: {e}"))
    }

    pub async fn get_template(&self, template_id: Uuid) -> Result<AgentTemplate, String> {
        sqlx::query_as!(
            AgentTemplate,
            "SELECT id, name, instructions, version, created_by, created_at, updated_at \
             FROM agent_templates WHERE id = $1",
            template_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("get_template: {e}"))
    }

    pub async fn list_templates(&self) -> Result<Vec<AgentTemplate>, String> {
        sqlx::query_as!(
            AgentTemplate,
            "SELECT id, name, instructions, version, created_by, created_at, updated_at \
             FROM agent_templates ORDER BY updated_at DESC",
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_templates: {e}"))
    }

    /// Apply a template to a set of agents (or all agents using any version of it).
    pub async fn deploy_template(
        &self,
        template_id: Uuid,
        agent_ids: &[Uuid],
    ) -> Result<u64, String> {
        let result = if agent_ids.is_empty() {
            sqlx::query!(
                "UPDATE agent_registry SET template_id = $1, updated_at = NOW()",
                template_id,
            )
            .execute(&self.db)
            .await
        } else {
            sqlx::query!(
                "UPDATE agent_registry SET template_id = $1, updated_at = NOW() \
                 WHERE id = ANY($2)",
                template_id,
                agent_ids,
            )
            .execute(&self.db)
            .await
        };
        result
            .map(|r| r.rows_affected())
            .map_err(|e| format!("deploy_template: {e}"))
    }

    // ── Audit export ──────────────────────────────────────────────────────────

    /// Returns all intervention logs + approval decisions for a given agent,
    /// ordered by timestamp — ready for CSV/JSON export.
    pub async fn audit_export(
        &self,
        agent_id: Uuid,
    ) -> Result<Vec<serde_json::Value>, String> {
        let interventions = self.list_interventions(agent_id).await?;
        let approvals = sqlx::query_as!(
            ApprovalQueueItem,
            r#"
            SELECT id, agent_id, task_id, reason, projected_cost_cngn,
                   decision, decided_by, decided_at, created_at
            FROM agent_approval_queue
            WHERE agent_id = $1
            ORDER BY created_at DESC
            "#,
            agent_id,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("audit_export approvals: {e}"))?;

        let mut rows: Vec<serde_json::Value> = interventions
            .into_iter()
            .map(|i| {
                serde_json::json!({
                    "timestamp": i.created_at,
                    "agent_id": i.agent_id,
                    "event_kind": "intervention",
                    "actor": i.performed_by,
                    "detail": { "action": i.action, "reason": i.reason }
                })
            })
            .chain(approvals.into_iter().map(|a| {
                serde_json::json!({
                    "timestamp": a.decided_at.unwrap_or(a.created_at),
                    "agent_id": a.agent_id,
                    "event_kind": "approval_decision",
                    "actor": a.decided_by.unwrap_or_else(|| "system".into()),
                    "detail": {
                        "task_id": a.task_id,
                        "decision": a.decision,
                        "projected_cost_cngn": a.projected_cost_cngn,
                        "reason": a.reason
                    }
                })
            }))
            .collect();

        rows.sort_by_key(|r| {
            r["timestamp"]
                .as_str()
                .unwrap_or("")
                .to_string()
        });
        Ok(rows)
    }
}
