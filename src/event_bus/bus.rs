use crate::event_bus::models::*;
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};

const MAX_RETRY_COUNT: i32 = 3;

/// In-process event bus backed by PostgreSQL for durability.
/// Publishes events to a broadcast channel for in-process consumers
/// and persists every event for audit trail and replay.
pub struct EventBus {
    pool: PgPool,
    sender: broadcast::Sender<PlatformEvent>,
}

impl EventBus {
    pub fn new(pool: PgPool) -> Arc<Self> {
        let (sender, _) = broadcast::channel(1024);
        Arc::new(Self { pool, sender })
    }

    /// Publish an event — persists to DB then broadcasts in-process
    pub async fn publish(&self, event: PlatformEvent) -> Result<()> {
        // Persist for durability and audit trail
        sqlx::query(
            r#"INSERT INTO event_records (
                event_id, event_type, aggregate_id, aggregate_type,
                payload, metadata, schema_version, published_at, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            ON CONFLICT (event_id) DO NOTHING"#,
        )
        .bind(event.event_id)
        .bind(&event.event_type)
        .bind(&event.aggregate_id)
        .bind(&event.aggregate_type)
        .bind(&event.payload)
        .bind(&event.metadata)
        .bind(&event.schema_version)
        .bind(event.published_at)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;

        info!(event_id = %event.event_id, event_type = %event.event_type, "Event published");

        // Broadcast to in-process subscribers (ignore send errors — no active receivers is fine)
        let _ = self.sender.send(event);
        Ok(())
    }

    /// Subscribe to the in-process broadcast channel
    pub fn subscribe(&self) -> broadcast::Receiver<PlatformEvent> {
        self.sender.subscribe()
    }

    /// Check idempotency — returns true if already processed by this consumer
    pub async fn is_already_processed(&self, event_id: Uuid, consumer_name: &str) -> Result<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM processed_events WHERE event_id = $1 AND consumer_name = $2)"
        )
        .bind(event_id)
        .bind(consumer_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    /// Mark an event as successfully processed (idempotency guard)
    pub async fn mark_processed(&self, event_id: Uuid, consumer_name: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO processed_events (event_id, consumer_name, processed_at) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING"
        )
        .bind(event_id)
        .bind(consumer_name)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Route a failed event to the dead-letter queue after max retries
    pub async fn dead_letter(&self, event_id: Uuid, consumer_name: &str, reason: &str) -> Result<()> {
        let existing: Option<i32> = sqlx::query_scalar(
            "SELECT retry_count FROM dead_letter_queue WHERE event_id = $1 AND consumer_name = $2"
        )
        .bind(event_id)
        .bind(consumer_name)
        .fetch_optional(&self.pool)
        .await?;

        match existing {
            Some(count) if count >= MAX_RETRY_COUNT => {
                warn!(event_id = %event_id, consumer = consumer_name, "Event dead-lettered after max retries");
                sqlx::query(
                    "UPDATE dead_letter_queue SET failure_reason = $1, last_attempted_at = $2 WHERE event_id = $3 AND consumer_name = $4"
                )
                .bind(reason)
                .bind(Utc::now())
                .bind(event_id)
                .bind(consumer_name)
                .execute(&self.pool)
                .await?;
            }
            Some(count) => {
                sqlx::query(
                    "UPDATE dead_letter_queue SET retry_count = $1, failure_reason = $2, last_attempted_at = $3 WHERE event_id = $4 AND consumer_name = $5"
                )
                .bind(count + 1)
                .bind(reason)
                .bind(Utc::now())
                .bind(event_id)
                .bind(consumer_name)
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query(
                    r#"INSERT INTO dead_letter_queue (dlq_id, event_id, consumer_name, failure_reason, retry_count, last_attempted_at, created_at)
                       VALUES ($1,$2,$3,$4,1,$5,$6)"#,
                )
                .bind(Uuid::new_v4())
                .bind(event_id)
                .bind(consumer_name)
                .bind(reason)
                .bind(Utc::now())
                .bind(Utc::now())
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Replay events for a given aggregate since a timestamp (for recovery)
    pub async fn replay_events(&self, aggregate_id: &str) -> Result<Vec<EventRecord>> {
        let events = sqlx::query_as::<_, EventRecord>(
            "SELECT * FROM event_records WHERE aggregate_id = $1 ORDER BY published_at ASC"
        )
        .bind(aggregate_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(events)
    }
}
