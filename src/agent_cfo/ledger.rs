use crate::agent_cfo::types::{InferenceEvent, LedgerQuery};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

pub struct ExpenditureLedger {
    db: PgPool,
}

impl ExpenditureLedger {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Sum of all costs recorded today for `agent_id`.
    pub async fn daily_spend(&self, agent_id: Uuid) -> Result<Decimal, String> {
        let row = sqlx::query!(
            r#"
            SELECT COALESCE(SUM(cost_cngn::numeric), 0)::text AS total
            FROM agent_inference_events
            WHERE agent_id = $1
              AND recorded_at >= date_trunc('day', NOW() AT TIME ZONE 'UTC')
            "#,
            agent_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("daily_spend query failed: {e}"))?;

        Decimal::from_str(row.total.as_deref().unwrap_or("0"))
            .map_err(|e| format!("decimal parse: {e}"))
    }

    /// Append one inference event and return the updated cumulative daily spend.
    pub async fn record(
        &self,
        agent_id: Uuid,
        task_id: Option<Uuid>,
        event_type: &str,
        model_used: Option<&str>,
        cost_cngn: &str,
        metadata: serde_json::Value,
    ) -> Result<InferenceEvent, String> {
        let daily = self.daily_spend(agent_id).await?;
        let cost = Decimal::from_str(cost_cngn).map_err(|e| format!("cost parse: {e}"))?;
        let cumulative = (daily + cost).to_string();

        sqlx::query_as!(
            InferenceEvent,
            r#"
            INSERT INTO agent_inference_events
                (id, agent_id, task_id, event_type, model_used, cost_cngn,
                 cumulative_daily_spend, metadata, recorded_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, NOW())
            RETURNING id, agent_id, task_id, event_type, model_used, cost_cngn,
                      cumulative_daily_spend, metadata, recorded_at
            "#,
            agent_id,
            task_id,
            event_type,
            model_used,
            cost_cngn,
            cumulative,
            metadata,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("ledger insert failed: {e}"))
    }

    /// Query the ledger with optional filters.
    pub async fn query(
        &self,
        q: &LedgerQuery,
    ) -> Result<Vec<InferenceEvent>, String> {
        sqlx::query_as!(
            InferenceEvent,
            r#"
            SELECT id, agent_id, task_id, event_type, model_used, cost_cngn,
                   cumulative_daily_spend, metadata, recorded_at
            FROM agent_inference_events
            WHERE ($1::uuid IS NULL OR agent_id = $1)
              AND ($2::uuid IS NULL OR task_id = $2)
            ORDER BY recorded_at DESC
            LIMIT $3 OFFSET $4
            "#,
            q.agent_id as Option<Uuid>,
            q.task_id as Option<Uuid>,
            q.page_size(),
            q.offset(),
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("ledger query failed: {e}"))
    }
}
