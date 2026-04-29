use crate::agent_swarm::types::{
    CreateSwarmTaskRequest, MicroTask, SwarmTaskRequest, SwarmTaskStatus,
};
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

pub struct DelegationEngine {
    db: PgPool,
}

impl DelegationEngine {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Manager agent decomposes a complex task and broadcasts micro-tasks
    /// with individual cNGN bounties to the swarm.
    pub async fn create_and_delegate(
        &self,
        req: CreateSwarmTaskRequest,
    ) -> Result<SwarmTaskRequest, String> {
        // Derive required_votes: majority of assignees
        let n = req.micro_tasks.len() as i32;
        let required_votes = req.required_votes.unwrap_or((n / 2) + 1).max(1);

        // Insert parent task
        let task = sqlx::query_as!(
            SwarmTaskRequest,
            r#"
            INSERT INTO swarm_task_requests
                (id, manager_agent_id, description, total_bounty_cngn,
                 status, required_votes, created_at)
            VALUES (gen_random_uuid(), $1, $2, $3, 'open'::swarm_task_status, $4, NOW())
            RETURNING id, manager_agent_id, description, total_bounty_cngn,
                      status AS "status: SwarmTaskStatus",
                      required_votes, result_payload, created_at, completed_at
            "#,
            req.manager_agent_id,
            req.description,
            req.total_bounty_cngn,
            required_votes,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("create swarm task: {e}"))?;

        // Insert micro-tasks
        for mt in &req.micro_tasks {
            sqlx::query!(
                r#"
                INSERT INTO swarm_micro_tasks
                    (id, swarm_task_id, assignee_agent_id, description,
                     bounty_cngn, status, created_at)
                VALUES (gen_random_uuid(), $1, $2, $3, $4, 'pending', NOW())
                "#,
                task.id,
                mt.assignee_agent_id,
                mt.description,
                mt.bounty_cngn,
            )
            .execute(&self.db)
            .await
            .map_err(|e| format!("insert micro_task: {e}"))?;
        }

        // Transition to in_progress
        sqlx::query!(
            "UPDATE swarm_task_requests SET status = 'in_progress' WHERE id = $1",
            task.id,
        )
        .execute(&self.db)
        .await
        .map_err(|e| format!("update status: {e}"))?;

        info!(
            task_id = %task.id,
            manager = %req.manager_agent_id,
            micro_tasks = n,
            bounty = %req.total_bounty_cngn,
            "📡 Swarm task delegated"
        );

        Ok(task)
    }

    /// Subordinate agent submits its result for a micro-task.
    pub async fn submit_micro_task(
        &self,
        micro_task_id: Uuid,
        agent_id: Uuid,
        result: serde_json::Value,
    ) -> Result<MicroTask, String> {
        sqlx::query_as!(
            MicroTask,
            r#"
            UPDATE swarm_micro_tasks
            SET status = 'submitted',
                result_payload = $3,
                submitted_at = NOW()
            WHERE id = $1 AND assignee_agent_id = $2
            RETURNING id, swarm_task_id, assignee_agent_id, description,
                      bounty_cngn, status, result_payload, submitted_at, created_at
            "#,
            micro_task_id,
            agent_id,
            result,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("submit_micro_task: {e}"))
    }

    pub async fn get_task(&self, task_id: Uuid) -> Result<SwarmTaskRequest, String> {
        sqlx::query_as!(
            SwarmTaskRequest,
            r#"
            SELECT id, manager_agent_id, description, total_bounty_cngn,
                   status AS "status: SwarmTaskStatus",
                   required_votes, result_payload, created_at, completed_at
            FROM swarm_task_requests WHERE id = $1
            "#,
            task_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("get_task: {e}"))
    }

    pub async fn list_micro_tasks(&self, task_id: Uuid) -> Result<Vec<MicroTask>, String> {
        sqlx::query_as!(
            MicroTask,
            r#"
            SELECT id, swarm_task_id, assignee_agent_id, description,
                   bounty_cngn, status, result_payload, submitted_at, created_at
            FROM swarm_micro_tasks WHERE swarm_task_id = $1
            ORDER BY created_at ASC
            "#,
            task_id,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_micro_tasks: {e}"))
    }
}
