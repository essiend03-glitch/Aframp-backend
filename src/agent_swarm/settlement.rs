use crate::agent_swarm::types::SwarmSettlement;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

pub struct SettlementEngine {
    db: PgPool,
}

impl SettlementEngine {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Record a pending x402 settlement for a completed micro-task.
    /// The actual Stellar transfer is submitted by the treasury/payment layer
    /// that watches the `swarm_settlements` table.
    pub async fn record_settlement(
        &self,
        swarm_task_id: Uuid,
        micro_task_id: Uuid,
        payer_agent_id: Uuid,
        payee_agent_id: Uuid,
        amount_cngn: &str,
    ) -> Result<SwarmSettlement, String> {
        sqlx::query_as!(
            SwarmSettlement,
            r#"
            INSERT INTO swarm_settlements
                (id, swarm_task_id, micro_task_id, payer_agent_id, payee_agent_id,
                 amount_cngn, status, created_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, 'pending', NOW())
            RETURNING id, swarm_task_id, micro_task_id, payer_agent_id, payee_agent_id,
                      amount_cngn, stellar_tx_hash, status, created_at, confirmed_at
            "#,
            swarm_task_id,
            micro_task_id,
            payer_agent_id,
            payee_agent_id,
            amount_cngn,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("record_settlement: {e}"))
    }

    /// Confirm a settlement once the Stellar tx is on-chain.
    pub async fn confirm_settlement(
        &self,
        settlement_id: Uuid,
        stellar_tx_hash: &str,
    ) -> Result<SwarmSettlement, String> {
        sqlx::query_as!(
            SwarmSettlement,
            r#"
            UPDATE swarm_settlements
            SET status = 'confirmed',
                stellar_tx_hash = $2,
                confirmed_at = NOW()
            WHERE id = $1
            RETURNING id, swarm_task_id, micro_task_id, payer_agent_id, payee_agent_id,
                      amount_cngn, stellar_tx_hash, status, created_at, confirmed_at
            "#,
            settlement_id,
            stellar_tx_hash,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("confirm_settlement: {e}"))
    }

    /// List all settlements for a swarm task.
    pub async fn list_by_task(&self, swarm_task_id: Uuid) -> Result<Vec<SwarmSettlement>, String> {
        sqlx::query_as!(
            SwarmSettlement,
            r#"
            SELECT id, swarm_task_id, micro_task_id, payer_agent_id, payee_agent_id,
                   amount_cngn, stellar_tx_hash, status, created_at, confirmed_at
            FROM swarm_settlements WHERE swarm_task_id = $1
            ORDER BY created_at ASC
            "#,
            swarm_task_id,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_settlements: {e}"))
    }

    /// Auto-settle all accepted micro-tasks for a completed swarm task.
    /// Called after consensus is reached.
    pub async fn settle_completed_task(
        &self,
        swarm_task_id: Uuid,
        manager_agent_id: Uuid,
    ) -> Result<Vec<SwarmSettlement>, String> {
        let micro_tasks = sqlx::query!(
            "SELECT id, assignee_agent_id, bounty_cngn \
             FROM swarm_micro_tasks \
             WHERE swarm_task_id = $1 AND status = 'submitted'",
            swarm_task_id,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("fetch micro_tasks: {e}"))?;

        let mut settlements = Vec::new();
        for mt in micro_tasks {
            let s = self
                .record_settlement(
                    swarm_task_id,
                    mt.id,
                    manager_agent_id,
                    mt.assignee_agent_id,
                    &mt.bounty_cngn.to_string(),
                )
                .await?;

            // Mark micro-task as accepted
            let _ = sqlx::query!(
                "UPDATE swarm_micro_tasks SET status = 'accepted' WHERE id = $1",
                mt.id,
            )
            .execute(&self.db)
            .await;

            info!(
                settlement_id = %s.id,
                payee = %mt.assignee_agent_id,
                amount = %mt.bounty_cngn,
                "💸 x402 settlement queued"
            );
            settlements.push(s);
        }
        Ok(settlements)
    }
}
