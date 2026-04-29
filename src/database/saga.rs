//! Cross-shard saga coordinator (Issue #423).
//!
//! # Pattern
//! For operations that touch rows on multiple shards (e.g. a transfer between
//! two wallets that live on different shards) we use a two-phase saga:
//!
//!   Phase 1 — Prepare:  each participant shard writes a "pending" saga record
//!                        and locks the affected rows.
//!   Phase 2 — Commit:   if all participants prepared successfully, each shard
//!                        applies the change and marks its record "committed".
//!             Rollback:  if any participant fails to prepare, all shards that
//!                        did prepare are told to rollback.
//!
//! This is intentionally simpler than full 2PC (no distributed lock manager).
//! The saga log is durable — a recovery job can replay incomplete sagas on
//! restart.

use std::sync::Arc;

use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::database::shard::ShardRouter;

// ---------------------------------------------------------------------------
// Saga state machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SagaState {
    Pending,
    Prepared,
    Committed,
    RolledBack,
    Failed,
}

impl SagaState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Prepared => "prepared",
            Self::Committed => "committed",
            Self::RolledBack => "rolled_back",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "prepared" => Self::Prepared,
            "committed" => Self::Committed,
            "rolled_back" => Self::RolledBack,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

// ---------------------------------------------------------------------------
// Participant: one shard's role in a saga
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SagaParticipant {
    pub shard_id: i16,
    pub shard_key: String,
    /// SQL to execute during the Prepare phase (idempotent, writes "pending" state).
    pub prepare_sql: String,
    /// SQL to execute during the Commit phase.
    pub commit_sql: String,
    /// SQL to execute during the Rollback phase.
    pub rollback_sql: String,
}

// ---------------------------------------------------------------------------
// Saga
// ---------------------------------------------------------------------------

pub struct Saga {
    pub id: Uuid,
    pub participants: Vec<SagaParticipant>,
    pub state: SagaState,
}

impl Saga {
    pub fn new(participants: Vec<SagaParticipant>) -> Self {
        Self {
            id: Uuid::new_v4(),
            participants,
            state: SagaState::Pending,
        }
    }
}

// ---------------------------------------------------------------------------
// Coordinator
// ---------------------------------------------------------------------------

pub struct SagaCoordinator {
    router: Arc<ShardRouter>,
}

impl SagaCoordinator {
    pub fn new(router: Arc<ShardRouter>) -> Self {
        Self { router }
    }

    /// Execute a saga: prepare all participants, then commit or rollback.
    pub async fn execute(&self, mut saga: Saga) -> SagaState {
        info!(saga_id=%saga.id, participants=%saga.participants.len(), "Saga starting");

        // --- Phase 1: Prepare ---
        let mut prepared: Vec<&SagaParticipant> = Vec::new();
        for participant in &saga.participants {
            match self.prepare(participant).await {
                Ok(_) => prepared.push(participant),
                Err(e) => {
                    warn!(saga_id=%saga.id, shard_id=%participant.shard_id, error=%e, "Prepare failed — rolling back");
                    self.rollback_all(&prepared, &saga.id).await;
                    saga.state = SagaState::RolledBack;
                    return saga.state;
                }
            }
        }

        saga.state = SagaState::Prepared;

        // --- Phase 2: Commit ---
        let mut all_committed = true;
        for participant in &saga.participants {
            if let Err(e) = self.commit(participant).await {
                error!(saga_id=%saga.id, shard_id=%participant.shard_id, error=%e, "Commit failed");
                all_committed = false;
            }
        }

        if all_committed {
            info!(saga_id=%saga.id, "Saga committed");
            saga.state = SagaState::Committed;
        } else {
            // Partial commit — log for manual recovery; do not attempt rollback
            // of already-committed shards (that would cause double-spend).
            error!(saga_id=%saga.id, "Saga partially committed — manual recovery required");
            saga.state = SagaState::Failed;
        }

        saga.state
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    async fn prepare(&self, p: &SagaParticipant) -> Result<(), String> {
        let pool = self.router.pool_for_write(&p.shard_key).await
            .map_err(|e| e.to_string())?;
        sqlx::query(&p.prepare_sql)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn commit(&self, p: &SagaParticipant) -> Result<(), String> {
        let pool = self.router.pool_for_write(&p.shard_key).await
            .map_err(|e| e.to_string())?;
        sqlx::query(&p.commit_sql)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn rollback_all(&self, prepared: &[&SagaParticipant], saga_id: &Uuid) {
        for p in prepared {
            let pool = match self.router.pool_for_write(&p.shard_key).await {
                Ok(pool) => pool,
                Err(e) => {
                    error!(saga_id=%saga_id, shard_id=%p.shard_id, error=%e, "Cannot get pool for rollback");
                    continue;
                }
            };
            if let Err(e) = sqlx::query(&p.rollback_sql).execute(&pool).await {
                error!(saga_id=%saga_id, shard_id=%p.shard_id, error=%e, "Rollback failed");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests (pure state-machine logic — no DB required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saga_state_transitions() {
        assert_eq!(SagaState::from_str("pending"), SagaState::Pending);
        assert_eq!(SagaState::from_str("prepared"), SagaState::Prepared);
        assert_eq!(SagaState::from_str("committed"), SagaState::Committed);
        assert_eq!(SagaState::from_str("rolled_back"), SagaState::RolledBack);
        assert_eq!(SagaState::from_str("failed"), SagaState::Failed);
        assert_eq!(SagaState::from_str("garbage"), SagaState::Pending);
    }

    #[test]
    fn test_saga_state_as_str_roundtrip() {
        for state in [
            SagaState::Pending,
            SagaState::Prepared,
            SagaState::Committed,
            SagaState::RolledBack,
            SagaState::Failed,
        ] {
            assert_eq!(SagaState::from_str(state.as_str()), state);
        }
    }

    #[test]
    fn test_saga_new_starts_pending() {
        let saga = Saga::new(vec![]);
        assert_eq!(saga.state, SagaState::Pending);
    }

    #[test]
    fn test_saga_new_generates_unique_ids() {
        let a = Saga::new(vec![]);
        let b = Saga::new(vec![]);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_participant_fields() {
        let p = SagaParticipant {
            shard_id: 1,
            shard_key: "wallet_abc".to_string(),
            prepare_sql: "UPDATE t SET status='pending' WHERE id=$1".to_string(),
            commit_sql: "UPDATE t SET status='done' WHERE id=$1".to_string(),
            rollback_sql: "UPDATE t SET status='cancelled' WHERE id=$1".to_string(),
        };
        assert_eq!(p.shard_id, 1);
        assert_eq!(p.shard_key, "wallet_abc");
    }
}
