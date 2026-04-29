//! Database shard router (Issue #423).
//!
//! # Design
//! - Sharding key: `wallet_address` (or any `&str` key).
//! - Algorithm: `shard_id = fnv1a_64(key) % active_shard_count`.
//!   FNV-1a is fast, dependency-free, and produces a uniform distribution.
//! - One `PgPool` per shard, created lazily on first use.
//! - Hot-reload: call `ShardRouter::reload(coordinator_pool)` to pick up
//!   changes to `shard_registry` without restarting the process.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::database::error::DatabaseError;

// ---------------------------------------------------------------------------
// Shard descriptor (mirrors shard_registry row)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShardConfig {
    pub shard_id: i16,
    pub dsn: String,
    pub status: ShardStatus,
    pub weight: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardStatus {
    Active,
    Draining, // accepts reads, rejects new writes
    Offline,
}

impl ShardStatus {
    fn from_str(s: &str) -> Self {
        match s {
            "draining" => Self::Draining,
            "offline" => Self::Offline,
            _ => Self::Active,
        }
    }

    pub fn accepts_writes(&self) -> bool {
        *self == Self::Active
    }
}

// ---------------------------------------------------------------------------
// Inner state (behind RwLock for hot-reload)
// ---------------------------------------------------------------------------

struct RouterState {
    shards: Vec<ShardConfig>,
    pools: HashMap<i16, PgPool>,
}

impl RouterState {
    /// Return only shards that accept writes, sorted by shard_id for
    /// deterministic modulo routing.
    fn active_shards(&self) -> Vec<&ShardConfig> {
        let mut v: Vec<&ShardConfig> = self
            .shards
            .iter()
            .filter(|s| s.status.accepts_writes())
            .collect();
        v.sort_by_key(|s| s.shard_id);
        v
    }
}

// ---------------------------------------------------------------------------
// ShardRouter
// ---------------------------------------------------------------------------

pub struct ShardRouter {
    /// Pool pointing at the coordinator DB (holds shard_registry).
    coordinator: PgPool,
    state: RwLock<RouterState>,
    max_connections_per_shard: u32,
}

impl ShardRouter {
    /// Build a router by reading `shard_registry` from the coordinator pool.
    pub async fn new(coordinator: PgPool, max_connections_per_shard: u32) -> Result<Arc<Self>, DatabaseError> {
        let shards = Self::fetch_shards(&coordinator).await?;
        let mut pools = HashMap::new();

        for shard in &shards {
            if shard.status != ShardStatus::Offline {
                let pool = Self::build_pool(&shard.dsn, max_connections_per_shard).await?;
                pools.insert(shard.shard_id, pool);
            }
        }

        info!(shard_count = shards.len(), "ShardRouter initialised");

        Ok(Arc::new(Self {
            coordinator,
            state: RwLock::new(RouterState { shards, pools }),
            max_connections_per_shard,
        }))
    }

    /// Re-read `shard_registry` and open pools for any new shards.
    /// Existing pools are reused; removed/offline shards are dropped.
    pub async fn reload(&self) -> Result<(), DatabaseError> {
        let shards = Self::fetch_shards(&self.coordinator).await?;
        let mut state = self.state.write().await;

        for shard in &shards {
            if shard.status != ShardStatus::Offline && !state.pools.contains_key(&shard.shard_id) {
                match Self::build_pool(&shard.dsn, self.max_connections_per_shard).await {
                    Ok(pool) => {
                        state.pools.insert(shard.shard_id, pool);
                        info!(shard_id = shard.shard_id, "New shard pool opened");
                    }
                    Err(e) => warn!(shard_id = shard.shard_id, error=%e, "Failed to open shard pool"),
                }
            }
        }

        // Remove pools for shards that are now offline or removed.
        let live_ids: std::collections::HashSet<i16> = shards
            .iter()
            .filter(|s| s.status != ShardStatus::Offline)
            .map(|s| s.shard_id)
            .collect();
        state.pools.retain(|id, _| live_ids.contains(id));
        state.shards = shards;

        info!("ShardRouter reloaded");
        Ok(())
    }

    /// Route a shard key to the correct pool (read-only access).
    pub async fn pool_for_key(&self, key: &str) -> Result<PgPool, DatabaseError> {
        let state = self.state.read().await;
        let shard_id = self.shard_id_for_key_inner(key, &state);
        state
            .pools
            .get(&shard_id)
            .cloned()
            .ok_or_else(|| DatabaseError::from_message(&format!("No pool for shard {}", shard_id)))
    }

    /// Route a shard key to the correct pool for writes.
    /// Returns an error if the target shard is draining.
    pub async fn pool_for_write(&self, key: &str) -> Result<PgPool, DatabaseError> {
        let state = self.state.read().await;
        let shard_id = self.shard_id_for_key_inner(key, &state);
        let shard = state.shards.iter().find(|s| s.shard_id == shard_id);
        if let Some(s) = shard {
            if !s.status.accepts_writes() {
                return Err(DatabaseError::from_message(&format!(
                    "Shard {} is draining — writes rejected", shard_id
                )));
            }
        }
        state
            .pools
            .get(&shard_id)
            .cloned()
            .ok_or_else(|| DatabaseError::from_message(&format!("No pool for shard {}", shard_id)))
    }

    /// Deterministically compute the shard ID for a key.
    pub async fn shard_id_for_key(&self, key: &str) -> i16 {
        let state = self.state.read().await;
        self.shard_id_for_key_inner(key, &state)
    }

    /// Return all active pools (for cross-shard fan-out reads).
    pub async fn all_active_pools(&self) -> Vec<(i16, PgPool)> {
        let state = self.state.read().await;
        state
            .active_shards()
            .iter()
            .filter_map(|s| state.pools.get(&s.shard_id).map(|p| (s.shard_id, p.clone())))
            .collect()
    }

    /// Return the coordinator pool (for shard_registry queries).
    pub fn coordinator(&self) -> &PgPool {
        &self.coordinator
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn shard_id_for_key_inner(&self, key: &str, state: &RouterState) -> i16 {
        let active = state.active_shards();
        if active.is_empty() {
            return 0;
        }
        let hash = fnv1a_64(key.as_bytes());
        let idx = (hash % active.len() as u64) as usize;
        active[idx].shard_id
    }

    async fn fetch_shards(pool: &PgPool) -> Result<Vec<ShardConfig>, DatabaseError> {
        let rows: Vec<(i16, String, String, i16)> = sqlx::query_as(
            "SELECT shard_id, dsn, status, weight FROM shard_registry ORDER BY shard_id",
        )
        .fetch_all(pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(rows
            .into_iter()
            .map(|(id, dsn, status, weight)| ShardConfig {
                shard_id: id,
                dsn,
                status: ShardStatus::from_str(&status),
                weight,
            })
            .collect())
    }

    async fn build_pool(dsn: &str, max_connections: u32) -> Result<PgPool, DatabaseError> {
        PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(dsn)
            .await
            .map_err(DatabaseError::from_sqlx)
    }
}

// ---------------------------------------------------------------------------
// FNV-1a 64-bit hash (no external dependency)
// ---------------------------------------------------------------------------

/// FNV-1a 64-bit hash. Fast, uniform, dependency-free.
pub fn fnv1a_64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;
    let mut hash = OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// DatabaseError helper (extend existing type without modifying it)
// ---------------------------------------------------------------------------

impl DatabaseError {
    pub fn from_message(msg: &str) -> Self {
        use crate::database::error::DatabaseErrorKind;
        Self {
            kind: DatabaseErrorKind::Unknown { message: msg.to_string() },
            context: None,
            is_retryable: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fnv1a_deterministic() {
        assert_eq!(fnv1a_64(b"wallet_abc"), fnv1a_64(b"wallet_abc"));
    }

    #[test]
    fn test_fnv1a_different_keys_differ() {
        assert_ne!(fnv1a_64(b"wallet_a"), fnv1a_64(b"wallet_b"));
    }

    #[test]
    fn test_routing_determinism() {
        // Same key always maps to the same shard index.
        let shards = vec![
            ShardConfig { shard_id: 0, dsn: "".into(), status: ShardStatus::Active, weight: 1 },
            ShardConfig { shard_id: 1, dsn: "".into(), status: ShardStatus::Active, weight: 1 },
            ShardConfig { shard_id: 2, dsn: "".into(), status: ShardStatus::Active, weight: 1 },
        ];
        let n = shards.len() as u64;
        let route = |key: &str| -> i16 {
            let hash = fnv1a_64(key.as_bytes());
            let idx = (hash % n) as usize;
            shards[idx].shard_id
        };

        let id1 = route("GTEST_WALLET_001");
        let id2 = route("GTEST_WALLET_001");
        assert_eq!(id1, id2, "Same key must always route to same shard");
    }

    #[test]
    fn test_routing_distribution() {
        // 1000 random-ish keys should spread across 4 shards with no shard
        // receiving more than 40% of the load (rough uniformity check).
        let n_shards = 4u64;
        let mut counts = [0u32; 4];
        for i in 0u32..1000 {
            let key = format!("wallet_{:08x}", i);
            let idx = (fnv1a_64(key.as_bytes()) % n_shards) as usize;
            counts[idx] += 1;
        }
        for (i, &c) in counts.iter().enumerate() {
            assert!(
                c < 400,
                "Shard {} received {} / 1000 keys — distribution too skewed",
                i, c
            );
        }
    }

    #[test]
    fn test_draining_shard_excluded_from_active() {
        let shards = vec![
            ShardConfig { shard_id: 0, dsn: "".into(), status: ShardStatus::Active, weight: 1 },
            ShardConfig { shard_id: 1, dsn: "".into(), status: ShardStatus::Draining, weight: 1 },
        ];
        let active: Vec<_> = shards.iter().filter(|s| s.status.accepts_writes()).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].shard_id, 0);
    }

    #[test]
    fn test_offline_shard_excluded() {
        let s = ShardConfig { shard_id: 2, dsn: "".into(), status: ShardStatus::Offline, weight: 1 };
        assert!(!s.status.accepts_writes());
    }

    #[test]
    fn test_shard_status_from_str() {
        assert_eq!(ShardStatus::from_str("active"), ShardStatus::Active);
        assert_eq!(ShardStatus::from_str("draining"), ShardStatus::Draining);
        assert_eq!(ShardStatus::from_str("offline"), ShardStatus::Offline);
        assert_eq!(ShardStatus::from_str("unknown"), ShardStatus::Active);
    }
}
