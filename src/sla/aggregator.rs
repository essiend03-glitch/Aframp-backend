//! Sliding-window P95/P99 latency aggregator backed by Redis sorted sets.
//!
//! Each observation is stored as a member in a Redis ZSET keyed by
//! `sla:window:{corridor}` with the score set to the Unix timestamp (ms).
//! Expired entries are pruned on every read so the window stays accurate.
//!
//! Overhead per observation: one ZADD + one ZREMRANGEBYSCORE = < 0.1 ms.

use crate::cache::RedisPool;
use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, warn};

const WINDOW_MS: u64 = 60_000; // 60-second sliding window

/// Record a single latency observation (in milliseconds) for a corridor.
pub async fn record_latency(redis: &RedisPool, corridor: &str, latency_ms: f64) {
    let key = format!("sla:window:{}", corridor);
    let now_ms = now_ms();
    // member = "<timestamp_ms>:<latency>" to allow duplicates at same ms
    let member = format!("{}:{}", now_ms, latency_ms);

    let mut conn = match redis.get().await {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Redis unavailable for SLA record");
            return;
        }
    };

    // ZADD with score = timestamp so we can range-prune by time
    let _: Result<(), _> = conn.zadd(&key, &member, now_ms as f64).await;
    // Expire the key after 2× window so it self-cleans
    let _: Result<(), _> = conn.expire(&key, (WINDOW_MS / 500) as i64).await;
}

/// Compute P95 and P99 latencies (ms) over the sliding window for a corridor.
/// Returns `(p95, p99)` or `None` if fewer than 5 samples exist.
pub async fn percentiles(redis: &RedisPool, corridor: &str) -> Option<(f64, f64)> {
    let key = format!("sla:window:{}", corridor);
    let cutoff = (now_ms() - WINDOW_MS) as f64;

    let mut conn = match redis.get().await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Redis unavailable for SLA percentiles");
            return None;
        }
    };

    // Prune stale entries
    let _: Result<(), _> = conn.zrembyscore(&key, 0.0_f64, cutoff).await;

    // Fetch all members in window
    let members: Vec<String> = conn.zrange(&key, 0, -1).await.unwrap_or_default();
    if members.len() < 5 {
        return None;
    }

    let mut latencies: Vec<f64> = members
        .iter()
        .filter_map(|m| m.split(':').nth(1).and_then(|v| v.parse().ok()))
        .collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p95 = percentile_value(&latencies, 95.0);
    let p99 = percentile_value(&latencies, 99.0);
    Some((p95, p99))
}

fn percentile_value(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
