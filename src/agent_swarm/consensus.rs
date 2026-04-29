use crate::agent_swarm::types::{ConsensusOutcome, ConsensusVote, SwarmTaskStatus};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

pub struct ConsensusEngine {
    db: PgPool,
}

impl ConsensusEngine {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Deterministically hash a result payload for vote comparison.
    pub fn hash_result(payload: &serde_json::Value) -> String {
        let canonical = serde_json::to_string(payload).unwrap_or_default();
        format!("{:x}", Sha256::digest(canonical.as_bytes()))
    }

    /// Cast a vote for a swarm task result.
    /// Returns the vote record and immediately checks for consensus.
    pub async fn cast_vote(
        &self,
        swarm_task_id: Uuid,
        voter_agent_id: Uuid,
        result_hash: &str,
    ) -> Result<(ConsensusVote, ConsensusOutcome), String> {
        // Idempotent — one vote per agent per task
        let vote = sqlx::query_as!(
            ConsensusVote,
            r#"
            INSERT INTO swarm_consensus_votes
                (id, swarm_task_id, voter_agent_id, result_hash, cast_at)
            VALUES (gen_random_uuid(), $1, $2, $3, NOW())
            ON CONFLICT (swarm_task_id, voter_agent_id) DO UPDATE
                SET result_hash = EXCLUDED.result_hash, cast_at = NOW()
            RETURNING id, swarm_task_id, voter_agent_id, result_hash, cast_at
            "#,
            swarm_task_id,
            voter_agent_id,
            result_hash,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("cast_vote: {e}"))?;

        let outcome = self.check_consensus(swarm_task_id).await?;
        Ok((vote, outcome))
    }

    /// Check whether majority consensus has been reached.
    /// If yes, commits the winning result to the parent task and marks it Completed.
    pub async fn check_consensus(&self, swarm_task_id: Uuid) -> Result<ConsensusOutcome, String> {
        // Fetch required_votes threshold
        let task = sqlx::query!(
            "SELECT required_votes FROM swarm_task_requests WHERE id = $1",
            swarm_task_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("fetch task: {e}"))?;

        // Find the result_hash with the most votes
        let top = sqlx::query!(
            r#"
            SELECT result_hash, COUNT(*) AS vote_count
            FROM swarm_consensus_votes
            WHERE swarm_task_id = $1
            GROUP BY result_hash
            ORDER BY vote_count DESC
            LIMIT 1
            "#,
            swarm_task_id,
        )
        .fetch_optional(&self.db)
        .await
        .map_err(|e| format!("tally votes: {e}"))?;

        let (winning_hash, vote_count) = match top {
            Some(r) => (Some(r.result_hash), r.vote_count.unwrap_or(0)),
            None => (None, 0),
        };

        let reached = vote_count >= task.required_votes as i64;

        if reached {
            if let Some(ref hash) = winning_hash {
                // Commit result to parent task
                sqlx::query!(
                    r#"
                    UPDATE swarm_task_requests
                    SET status = 'completed'::swarm_task_status,
                        result_payload = jsonb_build_object('winning_hash', $2::text),
                        completed_at = NOW()
                    WHERE id = $1 AND status != 'completed'
                    "#,
                    swarm_task_id,
                    hash,
                )
                .execute(&self.db)
                .await
                .map_err(|e| format!("commit result: {e}"))?;

                info!(
                    task_id = %swarm_task_id,
                    hash = %hash,
                    votes = vote_count,
                    "✅ Swarm consensus reached"
                );
            }
        }

        Ok(ConsensusOutcome {
            swarm_task_id,
            reached,
            winning_hash,
            vote_count,
            required_votes: task.required_votes,
        })
    }

    pub async fn list_votes(&self, swarm_task_id: Uuid) -> Result<Vec<ConsensusVote>, String> {
        sqlx::query_as!(
            ConsensusVote,
            "SELECT id, swarm_task_id, voter_agent_id, result_hash, cast_at \
             FROM swarm_consensus_votes WHERE swarm_task_id = $1 ORDER BY cast_at ASC",
            swarm_task_id,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_votes: {e}"))
    }
}
