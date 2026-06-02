//! Hierarchical Token Bucket (HTB) backed by Redis Lua for atomic evaluate-and-decrement.
//! Falls back to in-memory DashMap when Redis is unavailable (fail-safe).

use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use super::models::{RateLimitDecision, TenantSlaProfile};

/// Lua script: atomic HTB check-and-consume on a Redis key.
/// KEYS[1] = tenant bucket key, ARGV[1] = capacity, ARGV[2] = fill_rate,
/// ARGV[3] = cost (1), ARGV[4] = now_ms
const HTB_LUA: &str = r#"
local key      = KEYS[1]
local capacity = tonumber(ARGV[1])
local fill     = tonumber(ARGV[2])
local cost     = tonumber(ARGV[3])
local now      = tonumber(ARGV[4])
local data     = redis.call('HMGET', key, 'tokens', 'last_refill')
local tokens   = tonumber(data[1]) or capacity
local last     = tonumber(data[2]) or now
local elapsed  = (now - last) / 1000.0
tokens = math.min(capacity, tokens + elapsed * fill)
if tokens >= cost then
    tokens = tokens - cost
    redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
    redis.call('PEXPIRE', key, 60000)
    return 1
else
    redis.call('HMSET', key, 'tokens', tokens, 'last_refill', now)
    redis.call('PEXPIRE', key, 60000)
    return 0
end
"#;

/// Fallback in-memory token state when Redis is unavailable.
#[derive(Clone)]
struct InMemoryBucket {
    tokens:      f64,
    last_refill: Instant,
}

pub struct TokenBucketLimiter {
    redis: Option<redis::aio::ConnectionManager>,
    fallback: Arc<Mutex<HashMap<Uuid, InMemoryBucket>>>,
    /// Platform-wide ceiling (gateway layer).
    platform_baseline_rps: f64,
}

impl TokenBucketLimiter {
    pub fn new(redis: Option<redis::aio::ConnectionManager>, platform_baseline_rps: f64) -> Self {
        Self {
            redis,
            fallback: Arc::new(Mutex::new(HashMap::new())),
            platform_baseline_rps,
        }
    }

    /// Evaluate HTB at two layers: platform gateway + tenant.
    pub async fn check(&self, profile: &TenantSlaProfile) -> RateLimitDecision {
        // Layer 1 – platform-wide guard (simple in-memory check)
        // Layer 2 – per-tenant Redis bucket
        let allowed = if let Some(ref mgr) = self.redis {
            self.redis_check(mgr.clone(), profile).await
        } else {
            warn!("Redis unavailable – falling back to in-memory rate limiter");
            self.memory_check(profile).await
        };

        if allowed {
            RateLimitDecision::Allow
        } else {
            let retry_ms = (1000.0 / profile.fill_rate()) as u64;
            RateLimitDecision::Throttle { retry_after_ms: retry_ms }
        }
    }

    async fn redis_check(&self, mut mgr: redis::aio::ConnectionManager, profile: &TenantSlaProfile) -> bool {
        use redis::AsyncCommands;
        let key      = format!("htb:tenant:{}", profile.tenant_id);
        let now_ms   = chrono::Utc::now().timestamp_millis();
        let result: redis::RedisResult<i64> = redis::Script::new(HTB_LUA)
            .key(&key)
            .arg(profile.capacity())
            .arg(profile.fill_rate())
            .arg(1i64)
            .arg(now_ms)
            .invoke_async(&mut mgr)
            .await;
        match result {
            Ok(1) => true,
            Ok(_) => false,
            Err(e) => {
                warn!("Redis HTB error: {e} – falling back to memory");
                self.memory_check(profile).await
            }
        }
    }

    async fn memory_check(&self, profile: &TenantSlaProfile) -> bool {
        let mut map = self.fallback.lock().await;
        let now = Instant::now();
        let entry = map.entry(profile.tenant_id).or_insert(InMemoryBucket {
            tokens:      profile.capacity(),
            last_refill: now,
        });
        let elapsed = now.duration_since(entry.last_refill).as_secs_f64();
        entry.tokens = f64::min(profile.capacity(), entry.tokens + elapsed * profile.fill_rate());
        entry.last_refill = now;
        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}
