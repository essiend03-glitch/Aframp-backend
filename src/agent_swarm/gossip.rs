use crate::agent_swarm::types::GossipEntry;
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{info, warn};

pub struct GossipStore {
    db: PgPool,
}

impl GossipStore {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Push a gossip entry. Accepted only if `version` is strictly greater
    /// than the stored version (last-write-wins / Lamport clock).
    pub async fn push(&self, req: &crate::agent_swarm::types::GossipPushRequest) -> Result<GossipEntry, String> {
        sqlx::query_as!(
            GossipEntry,
            r#"
            INSERT INTO swarm_gossip_state (id, state_key, value, version, origin_peer_id, updated_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW())
            ON CONFLICT (state_key) DO UPDATE
                SET value          = EXCLUDED.value,
                    version        = EXCLUDED.version,
                    origin_peer_id = EXCLUDED.origin_peer_id,
                    updated_at     = NOW()
                WHERE swarm_gossip_state.version < EXCLUDED.version
            RETURNING id, state_key, value, version, origin_peer_id, updated_at
            "#,
            req.state_key,
            req.value,
            req.version,
            req.origin_peer_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("gossip push: {e}"))
    }

    /// Read the current value for a state key.
    pub async fn get(&self, state_key: &str) -> Result<Option<GossipEntry>, String> {
        sqlx::query_as!(
            GossipEntry,
            "SELECT id, state_key, value, version, origin_peer_id, updated_at \
             FROM swarm_gossip_state WHERE state_key = $1",
            state_key,
        )
        .fetch_optional(&self.db)
        .await
        .map_err(|e| format!("gossip get: {e}"))
    }

    /// Dump the full gossip table — used by agents joining the swarm to
    /// bootstrap their local state.
    pub async fn snapshot(&self) -> Result<Vec<GossipEntry>, String> {
        sqlx::query_as!(
            GossipEntry,
            "SELECT id, state_key, value, version, origin_peer_id, updated_at \
             FROM swarm_gossip_state ORDER BY state_key ASC",
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("gossip snapshot: {e}"))
    }

    /// Background worker: evicts stale gossip entries older than TTL.
    pub async fn run_eviction_worker(db: PgPool, mut shutdown: watch::Receiver<bool>) {
        let ttl_secs: i64 = std::env::var("SWARM_GOSSIP_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);
        let interval = Duration::from_secs(300);
        let mut ticker = tokio::time::interval(interval);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match sqlx::query!(
                        "DELETE FROM swarm_gossip_state \
                         WHERE updated_at < NOW() - ($1 || ' seconds')::interval",
                        ttl_secs.to_string(),
                    )
                    .execute(&db)
                    .await
                    {
                        Ok(r) if r.rows_affected() > 0 => {
                            info!(evicted = r.rows_affected(), "Gossip: evicted stale entries");
                        }
                        Err(e) => warn!(error = %e, "Gossip eviction failed"),
                        _ => {}
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Gossip eviction worker shutting down");
                        break;
                    }
                }
            }
        }
    }
}
