use crate::agent_swarm::types::{PeerListQuery, PeerTier, SwarmPeer};
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{info, warn};
use uuid::Uuid;

pub struct PeerDiscovery {
    db: PgPool,
}

impl PeerDiscovery {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Register or refresh a peer in the routing table.
    /// Returns the upserted peer record.
    pub async fn register_peer(
        &self,
        peer_id: &str,
        agent_id: Uuid,
        endpoint: &str,
    ) -> Result<SwarmPeer, String> {
        sqlx::query_as!(
            SwarmPeer,
            r#"
            INSERT INTO swarm_peers (id, peer_id, agent_id, endpoint, tier, reputation, last_seen_at, created_at)
            VALUES (gen_random_uuid(), $1, $2, $3, 'provisional', 50, NOW(), NOW())
            ON CONFLICT (peer_id) DO UPDATE
                SET endpoint = EXCLUDED.endpoint,
                    last_seen_at = NOW()
            RETURNING id, peer_id, agent_id, endpoint,
                      tier AS "tier: PeerTier",
                      reputation, last_seen_at, created_at
            "#,
            peer_id,
            agent_id,
            endpoint,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("register_peer: {e}"))
    }

    /// List peers with optional tier filter — used by agents to discover neighbours.
    pub async fn list_peers(&self, q: &PeerListQuery) -> Result<Vec<SwarmPeer>, String> {
        sqlx::query_as!(
            SwarmPeer,
            r#"
            SELECT id, peer_id, agent_id, endpoint,
                   tier AS "tier: PeerTier",
                   reputation, last_seen_at, created_at
            FROM swarm_peers
            WHERE tier != 'revoked'
              AND ($1::peer_tier IS NULL OR tier = $1)
            ORDER BY reputation DESC, last_seen_at DESC
            LIMIT $2 OFFSET $3
            "#,
            q.tier as Option<PeerTier>,
            q.page_size(),
            q.offset(),
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("list_peers: {e}"))
    }

    /// Promote a provisional peer to trusted after successful task completion.
    pub async fn promote_peer(&self, peer_id: &str, reputation_delta: i32) -> Result<(), String> {
        sqlx::query!(
            r#"
            UPDATE swarm_peers
            SET reputation = LEAST(100, reputation + $2),
                tier = CASE WHEN reputation + $2 >= 70 THEN 'trusted'::peer_tier ELSE tier END,
                last_seen_at = NOW()
            WHERE peer_id = $1
            "#,
            peer_id,
            reputation_delta,
        )
        .execute(&self.db)
        .await
        .map(|_| ())
        .map_err(|e| format!("promote_peer: {e}"))
    }

    /// Revoke a peer (ban from swarm).
    pub async fn revoke_peer(&self, peer_id: &str) -> Result<(), String> {
        sqlx::query!(
            "UPDATE swarm_peers SET tier = 'revoked' WHERE peer_id = $1",
            peer_id,
        )
        .execute(&self.db)
        .await
        .map(|_| ())
        .map_err(|e| format!("revoke_peer: {e}"))
    }

    /// Mark peers that haven't sent a heartbeat in `timeout` as stale
    /// (reputation penalty). Runs as a background sweep.
    pub async fn run_heartbeat_sweep(db: PgPool, mut shutdown: watch::Receiver<bool>) {
        let interval = Duration::from_secs(
            std::env::var("SWARM_HEARTBEAT_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120u64),
        );
        let stale_secs: i64 = std::env::var("SWARM_STALE_PEER_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);

        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let result = sqlx::query!(
                        r#"
                        UPDATE swarm_peers
                        SET reputation = GREATEST(0, reputation - 5)
                        WHERE last_seen_at < NOW() - ($1 || ' seconds')::interval
                          AND tier != 'revoked'
                        "#,
                        stale_secs.to_string(),
                    )
                    .execute(&db)
                    .await;

                    match result {
                        Ok(r) if r.rows_affected() > 0 => {
                            warn!(stale_peers = r.rows_affected(), "Swarm: penalised stale peers");
                        }
                        Err(e) => warn!(error = %e, "Swarm heartbeat sweep failed"),
                        _ => {}
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Swarm heartbeat sweep shutting down");
                        break;
                    }
                }
            }
        }
    }
}
