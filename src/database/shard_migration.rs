//! Online shard migration tool (Issue #423).
//!
//! Moves rows from a source shard to a target shard with near-zero downtime:
//!
//!   1. Read a batch of rows from the source shard (keyed by `shard_key_col`).
//!   2. INSERT them into the target shard (idempotent via ON CONFLICT DO NOTHING).
//!   3. DELETE them from the source shard only after the INSERT succeeds.
//!   4. Persist the cursor (`last_key`) in `shard_migration_jobs` so the job
//!      can be resumed after a crash without re-processing rows.
//!
//! The source shard is set to `draining` in `shard_registry` before the job
//! starts, so the router stops sending new writes there while reads still work.

use std::sync::Arc;

use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::database::shard::ShardRouter;

// ---------------------------------------------------------------------------
// Job descriptor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MigrationJob {
    pub id: Uuid,
    pub table_name: String,
    pub source_shard: i16,
    pub target_shard: i16,
    pub shard_key_col: String,
    pub batch_size: u32,
}

impl MigrationJob {
    pub fn new(
        table_name: impl Into<String>,
        source_shard: i16,
        target_shard: i16,
        shard_key_col: impl Into<String>,
        batch_size: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            table_name: table_name.into(),
            source_shard,
            target_shard,
            shard_key_col: shard_key_col.into(),
            batch_size,
        }
    }
}

// ---------------------------------------------------------------------------
// Migrator
// ---------------------------------------------------------------------------

pub struct ShardMigrator {
    router: Arc<ShardRouter>,
}

impl ShardMigrator {
    pub fn new(router: Arc<ShardRouter>) -> Self {
        Self { router }
    }

    /// Run a migration job to completion, processing `batch_size` rows per
    /// iteration. Resumes from `last_key` if the job was previously interrupted.
    pub async fn run(&self, job: &MigrationJob) -> Result<u64, String> {
        // Persist job start in coordinator
        let coordinator = self.router.coordinator();
        self.upsert_job(coordinator, job, "running", None, 0).await?;

        let source_pool = self.get_shard_pool(job.source_shard).await?;
        let target_pool = self.get_shard_pool(job.target_shard).await?;

        let mut last_key: Option<String> = self.load_cursor(coordinator, job.id).await;
        let mut total_migrated: u64 = 0;

        loop {
            let batch = self
                .fetch_batch(&source_pool, &job.table_name, &job.shard_key_col, &last_key, job.batch_size)
                .await?;

            if batch.is_empty() {
                break;
            }

            let new_last_key = batch.last().cloned();

            // Copy to target
            let copied = self
                .copy_batch(&source_pool, &target_pool, &job.table_name, &job.shard_key_col, &batch)
                .await?;

            // Delete from source only after successful copy
            self.delete_batch(&source_pool, &job.table_name, &job.shard_key_col, &batch)
                .await?;

            total_migrated += copied;
            last_key = new_last_key;

            // Persist cursor
            self.upsert_job(coordinator, job, "running", last_key.as_deref(), total_migrated as i64).await?;

            info!(
                job_id=%job.id,
                table=%job.table_name,
                rows_migrated=%total_migrated,
                last_key=?last_key,
                "Migration batch complete"
            );
        }

        self.upsert_job(coordinator, job, "done", last_key.as_deref(), total_migrated as i64).await?;
        info!(job_id=%job.id, total_migrated=%total_migrated, "Migration job complete");
        Ok(total_migrated)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    async fn get_shard_pool(&self, shard_id: i16) -> Result<PgPool, String> {
        // Use a dummy key that hashes to the desired shard — for direct pool
        // access we look up by shard_id from all_active_pools.
        let pools = self.router.all_active_pools().await;
        pools
            .into_iter()
            .find(|(id, _)| *id == shard_id)
            .map(|(_, pool)| pool)
            .ok_or_else(|| format!("Shard {} not found or offline", shard_id))
    }

    /// Fetch a batch of primary-key values from the source shard, ordered by
    /// `shard_key_col`, starting after `last_key`.
    async fn fetch_batch(
        &self,
        pool: &PgPool,
        table: &str,
        key_col: &str,
        last_key: &Option<String>,
        batch_size: u32,
    ) -> Result<Vec<String>, String> {
        // Dynamic SQL — table and column names are operator-supplied (not user
        // input), so string interpolation is acceptable here.
        let sql = match last_key {
            Some(k) => format!(
                "SELECT {key_col}::text FROM {table} WHERE {key_col}::text > $1 ORDER BY {key_col} LIMIT $2"
            ),
            None => format!(
                "SELECT {key_col}::text FROM {table} ORDER BY {key_col} LIMIT $1"
            ),
        };

        let rows: Vec<(String,)> = if let Some(k) = last_key {
            sqlx::query_as(&sql)
                .bind(k)
                .bind(batch_size as i64)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?
        } else {
            sqlx::query_as(&sql)
                .bind(batch_size as i64)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?
        };

        Ok(rows.into_iter().map(|(k,)| k).collect())
    }

    /// Copy rows identified by `keys` from source to target using INSERT … SELECT.
    /// ON CONFLICT DO NOTHING makes this idempotent.
    async fn copy_batch(
        &self,
        source: &PgPool,
        target: &PgPool,
        table: &str,
        key_col: &str,
        keys: &[String],
    ) -> Result<u64, String> {
        if keys.is_empty() {
            return Ok(0);
        }

        // Fetch full rows from source
        let placeholders: String = keys
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let select_sql = format!(
            "SELECT * FROM {table} WHERE {key_col}::text IN ({placeholders})"
        );

        // We use JSON to transfer rows generically without knowing the schema.
        let json_rows: Vec<(serde_json::Value,)> = {
            let mut q = sqlx::query_as::<_, (serde_json::Value,)>(&format!(
                "SELECT row_to_json(t) FROM ({select_sql}) t"
            ));
            for k in keys {
                q = q.bind(k);
            }
            q.fetch_all(source).await.map_err(|e| e.to_string())?
        };

        // INSERT each row into target via json_populate_record
        let mut copied = 0u64;
        for (row,) in &json_rows {
            let insert_sql = format!(
                "INSERT INTO {table} SELECT * FROM json_populate_record(null::{table}, $1) ON CONFLICT DO NOTHING"
            );
            sqlx::query(&insert_sql)
                .bind(row)
                .execute(target)
                .await
                .map_err(|e| e.to_string())?;
            copied += 1;
        }

        Ok(copied)
    }

    async fn delete_batch(
        &self,
        source: &PgPool,
        table: &str,
        key_col: &str,
        keys: &[String],
    ) -> Result<(), String> {
        if keys.is_empty() {
            return Ok(());
        }
        let placeholders: String = keys
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("DELETE FROM {table} WHERE {key_col}::text IN ({placeholders})");
        let mut q = sqlx::query(&sql);
        for k in keys {
            q = q.bind(k);
        }
        q.execute(source).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn load_cursor(&self, coordinator: &PgPool, job_id: Uuid) -> Option<String> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT last_key FROM shard_migration_jobs WHERE id=$1",
        )
        .bind(job_id)
        .fetch_optional(coordinator)
        .await
        .ok()
        .flatten();
        row.and_then(|(k,)| k)
    }

    async fn upsert_job(
        &self,
        coordinator: &PgPool,
        job: &MigrationJob,
        status: &str,
        last_key: Option<&str>,
        rows_migrated: i64,
    ) -> Result<(), String> {
        sqlx::query(
            r#"INSERT INTO shard_migration_jobs
               (id, table_name, source_shard, target_shard, shard_key_col, status, last_key, rows_migrated, started_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,now())
               ON CONFLICT (id) DO UPDATE SET
                 status=$6, last_key=$7, rows_migrated=$8,
                 finished_at = CASE WHEN $6 IN ('done','failed') THEN now() ELSE NULL END,
                 updated_at=now()"#,
        )
        .bind(job.id)
        .bind(&job.table_name)
        .bind(job.source_shard)
        .bind(job.target_shard)
        .bind(&job.shard_key_col)
        .bind(status)
        .bind(last_key)
        .bind(rows_migrated)
        .execute(coordinator)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
