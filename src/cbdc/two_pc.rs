use crate::cache::RedisPool;
use crate::cbdc::models::*;
use crate::cbdc::repository::CbdcRepository;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoPhaseLockState {
    None,
    Preparing,
    Prepared,
    Committing,
    Committed,
    RollingBack,
    RolledBack,
}

impl TwoPhaseLockState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TwoPhaseLockState::None => "none",
            TwoPhaseLockState::Preparing => "preparing",
            TwoPhaseLockState::Prepared => "prepared",
            TwoPhaseLockState::Committing => "committing",
            TwoPhaseLockState::Committed => "committed",
            TwoPhaseLockState::RollingBack => "rolling_back",
            TwoPhaseLockState::RolledBack => "rolled_back",
        }
    }
}

/// Two-Phase Commit (2PC) Lock Manager backed by Redis.
///
/// Guarantees that an asset cannot be released on the Stellar ledger until the
/// central bank node acknowledges permanent state finality. If communication
/// drops between preparation and commitment phases, the transaction is frozen
/// into a HELD_FOR_RECONCILIATION state.
pub struct TwoPhaseCommitManager {
    repo: Arc<CbdcRepository>,
    redis_pool: RedisPool,
    lock_ttl: Duration,
    heartbeat_interval: Duration,
    worker_id: String,
}

impl TwoPhaseCommitManager {
    pub fn new(
        repo: Arc<CbdcRepository>,
        redis_pool: RedisPool,
        config: &super::models::CbdcWorkerConfig,
    ) -> Self {
        Self {
            repo,
            redis_pool,
            lock_ttl: Duration::from_secs(config.two_phase_lock_ttl_secs),
            heartbeat_interval: Duration::from_secs(config.two_phase_heartbeat_interval_secs),
            worker_id: format!("cbdc-2pc-{}", uuid::Uuid::new_v4()),
        }
    }

    /// Attempts to acquire a distributed lock in Redis for the given swap.
    /// On success, creates a corresponding DB lock record.
    #[instrument(skip(self))]
    pub async fn acquire_lock(
        &self,
        swap_record_id: Uuid,
        gateway_id: Option<Uuid>,
        lock_key: &str,
    ) -> Result<TwoPcLock, String> {
        let redis_key = format!("cbdc:2pc:lock:{}", lock_key);

        // Try to acquire the Redis lock with NX (set if not exists)
        let mut conn = self.redis_pool.get().await.map_err(|e| format!("Redis pool error: {}", e))?;
        let result: Option<String> = redis::cmd("SET")
            .arg(&redis_key)
            .arg(&self.worker_id)
            .arg("NX")
            .arg("PX")
            .arg(self.lock_ttl.as_millis() as u64)
            .query_async(&mut *conn)
            .await
            .map_err(|e| format!("Redis lock acquire failed: {}", e))?;

        if result.is_none() {
            return Err(format!("Lock already held for key: {}", lock_key));
        }

        // Create persistent lock record in DB
        let db_lock = self
            .repo
            .create_2pc_lock(lock_key, swap_record_id, gateway_id, &self.worker_id, self.lock_ttl.as_secs())
            .await
            .map_err(|e| {
                let mut conn = self.redis_pool.get().await.ok();
                if let Some(ref mut conn) = conn {
                    let _: Result<(), _> = redis::cmd("DEL")
                        .arg(&redis_key)
                        .query_async(*conn)
                        .await;
                }
                format!("Failed to create 2PC lock record: {}", e)
            })?;

        info!(
            lock_id = %db_lock.id,
            lock_key = %lock_key,
            worker = %self.worker_id,
            "2PC lock acquired"
        );

        Ok(db_lock)
    }

    /// Marks the prepared phase: store prepared payload and advance lock state.
    #[instrument(skip(self, prepared_payload))]
    pub async fn prepare(
        &self,
        lock: &TwoPcLock,
        prepared_payload: &serde_json::Value,
    ) -> Result<TwoPcLock, String> {
        let updated = self
            .repo
            .update_2pc_prepared(lock.id, prepared_payload)
            .await
            .map_err(|e| format!("Failed to prepare 2PC lock: {}", e))?;

        info!(
            lock_id = %lock.id,
            "2PC: prepared phase complete"
        );

        Ok(updated)
    }

    /// Commits the transaction: advance lock to committing, then committed.
    #[instrument(skip(self, commit_payload))]
    pub async fn commit(
        &self,
        lock: &TwoPcLock,
        commit_payload: &serde_json::Value,
    ) -> Result<TwoPcLock, String> {
        // Stage 1: mark as committing
        let updated = self
            .repo
            .update_2pc_committing(lock.id, commit_payload)
            .await
            .map_err(|e| format!("Failed to start 2PC commit: {}", e))?;

        // Stage 2: mark as committed
        let updated = self
            .repo
            .update_2pc_committed(updated.id)
            .await
            .map_err(|e| format!("Failed to complete 2PC commit: {}", e))?;

        // Release Redis lock
        self.release_redis_lock(&lock.lock_key).await;

        info!(
            lock_id = %lock.id,
            "2PC: committed successfully"
        );

        Ok(updated)
    }

    /// Rolls back the transaction: execute rollback operations and update state.
    #[instrument(skip(self, rollback_payload))]
    pub async fn rollback(
        &self,
        lock: &TwoPcLock,
        rollback_payload: &serde_json::Value,
    ) -> Result<TwoPcLock, String> {
        let updated = self
            .repo
            .update_2pc_rolling_back(lock.id, rollback_payload)
            .await
            .map_err(|e| format!("Failed to start 2PC rollback: {}", e))?;

        let updated = self
            .repo
            .update_2pc_rolled_back(updated.id)
            .await
            .map_err(|e| format!("Failed to complete 2PC rollback: {}", e))?;

        // Release Redis lock
        self.release_redis_lock(&lock.lock_key).await;

        // Hold swap for reconciliation
        self.repo.hold_for_reconciliation(lock.swap_record_id).await.ok();

        warn!(
            lock_id = %lock.id,
            swap_id = %lock.swap_record_id,
            "2PC: rolled back — swap held for reconciliation"
        );

        Ok(updated)
    }

    /// Sends a heartbeat to keep the 2PC lock alive.
    #[instrument(skip(self))]
    pub async fn heartbeat(&self, lock_id: Uuid) -> Result<(), String> {
        self.repo
            .heartbeat_2pc_lock(lock_id)
            .await
            .map_err(|e| format!("2PC heartbeat failed: {}", e))
    }

    /// Recovers stale locks that were left in an incomplete state.
    #[instrument(skip(self))]
    pub async fn recover_stale_locks(&self) -> Result<Vec<TwoPcLock>, String> {
        let stale_locks = self
            .repo
            .find_stale_2pc_locks()
            .await
            .map_err(|e| format!("Failed to find stale 2PC locks: {}", e))?;

        for lock in &stale_locks {
            warn!(
                lock_id = %lock.id,
                state = %lock.lock_state,
                "Recovering stale 2PC lock"
            );

            // If in preparing/prepared state, attempt rollback
            if lock.lock_state == "preparing" || lock.lock_state == "prepared" {
                let rollback_payload = serde_json::json!({
                    "reason": "lock_timeout",
                    "recovered_at": chrono::Utc::now().to_rfc3339(),
                    "original_state": lock.lock_state,
                });
                self.rollback(lock, &rollback_payload).await?;
            }
        }

        Ok(stale_locks)
    }

    async fn release_redis_lock(&self, lock_key: &str) {
        let redis_key = format!("cbdc:2pc:lock:{}", lock_key);
        if let Ok(mut conn) = self.redis_pool.get().await {
            let _: Result<(), _> = redis::cmd("DEL")
                .arg(&redis_key)
                .query_async(&mut *conn)
                .await;
        }
    }
}
