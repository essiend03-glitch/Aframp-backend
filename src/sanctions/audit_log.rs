//! Immutable sanctions screening audit log — #419
//!
//! Every screening call is written to `sanctions_screening_log` with
//! `INSERT … ON CONFLICT DO NOTHING` to guarantee idempotency.
//! Rows are never updated or deleted (enforced by the DB trigger in the migration).

use sqlx::PgPool;
use uuid::Uuid;

use super::models::{ScreeningLogEntry, ScreeningResult};

pub struct AuditLog {
    pool: PgPool,
}

impl AuditLog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Append a screening result to the immutable log.
    /// Returns the persisted entry (or the existing one if already written).
    pub async fn append(&self, result: &ScreeningResult) -> Result<ScreeningLogEntry, anyhow::Error> {
        let id = Uuid::new_v4();
        let outcome = format!("{:?}", result.outcome);
        let matches_json = serde_json::to_value(&result.matches)?;

        // Use INSERT … ON CONFLICT DO NOTHING; if the row already exists
        // (idempotent retry), fetch it separately.
        sqlx::query!(
            r#"
            INSERT INTO sanctions_screening_log
                (id, transaction_id, outcome, matches_json, latency_ms, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
            "#,
            id,
            result.transaction_id,
            outcome,
            matches_json,
            result.latency_ms as i64,
            result.screened_at,
        )
        .execute(&self.pool)
        .await?;

        let entry = sqlx::query_as!(
            ScreeningLogEntry,
            r#"
            SELECT id, transaction_id, outcome, matches_json, latency_ms, created_at
            FROM sanctions_screening_log WHERE id = $1
            "#,
            id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(entry)
    }

    /// Fetch the most recent screening result for a transaction.
    pub async fn latest_for_transaction(
        &self,
        transaction_id: Uuid,
    ) -> Result<Option<ScreeningLogEntry>, anyhow::Error> {
        let entry = sqlx::query_as!(
            ScreeningLogEntry,
            r#"
            SELECT id, transaction_id, outcome, matches_json, latency_ms, created_at
            FROM sanctions_screening_log
            WHERE transaction_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            transaction_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(entry)
    }
}
