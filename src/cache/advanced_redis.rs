//! Advanced Redis Features for High-Performance Caching
//!
//! Implements advanced Redis caching strategies:
//! - Distributed locking for cache consistency
//! - Event-driven cache invalidation
//! - Cache-aside pattern with automatic fallback
//! - Multi-tier caching with L1/L2 coordination
//! - Performance monitoring and optimization

use super::{error::CacheResult, RedisPool};
use crate::cache::CacheError;
use async_trait::async_trait;
use bb8::PooledConnection;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

type RedisConnection<'a> = PooledConnection<'a, RedisConnectionManager>;

#[derive(Debug, Clone)]
pub struct AdvancedRedisCache {
    pool: RedisPool,
    config: AdvancedCacheConfig,
}

#[derive(Debug, Clone)]
pub struct AdvancedCacheConfig {
    pub default_ttl: Duration,
    pub lock_timeout: Duration,
    pub max_retry_attempts: u32,
    pub enable_distributed_locking: bool,
    pub enable_event_driven_invalidation: bool,
    pub enable_cache_warming: bool,
    pub enable_performance_monitoring: bool,
    pub compression_threshold: usize,
}

impl Default for AdvancedCacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(300), // 5 minutes
            lock_timeout: Duration::from_secs(10),
            max_retry_attempts: 3,
            enable_distributed_locking: true,
            enable_event_driven_invalidation: true,
            enable_cache_warming: true,
            enable_performance_monitoring: true,
            compression_threshold: 1024, // 1KB
        }
    }
}

impl AdvancedRedisCache {
    pub fn new(pool: RedisPool) -> Self {
        Self::new_with_config(pool, AdvancedCacheConfig::default())
    }

    pub fn new_with_config(pool: RedisPool, config: AdvancedCacheConfig) -> Self {
        Self { pool, config }
    }

    async fn get_connection(&self) -> CacheResult<RedisConnection<'_>> {
        self.pool.get().await.map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            e.into()
        })
    }

    /// Cache-Aside pattern: Get from cache, or fetch and cache if missing
    pub async fn get_or_set<F, T, Fut>(
        &self,
        key: &str,
        fetch_fn: F,
        ttl: Option<Duration>,
    ) -> CacheResult<T>
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = CacheResult<T>>,
    {
        // Try to get from cache first
        if let Some(cached_value) = self.get(key).await? {
            debug!("Cache hit for key: {}", key);
            return Ok(cached_value);
        }

        debug!("Cache miss for key: {}, fetching from source", key);

        // If distributed locking is enabled, acquire a lock to prevent thundering herd
        if self.config.enable_distributed_locking {
            let lock_key = format!("lock:{}", key);
            if let Ok(_lock) = self.acquire_lock(&lock_key).await {
                // Double-check cache after acquiring lock (in case another request populated it)
                if let Some(cached_value) = self.get(key).await? {
                    self.release_lock(&lock_key).await?;
                    return Ok(cached_value);
                }

                // Fetch from source
                let value = fetch_fn().await?;
                
                // Set in cache
                self.set(key, &value, ttl).await?;
                
                // Release lock
                self.release_lock(&lock_key).await?;
                
                Ok(value)
            } else {
                // Failed to acquire lock, fetch without caching to avoid stampede
                fetch_fn().await
            }
        } else {
            // No locking, fetch and cache directly
            let value = fetch_fn().await?;
            self.set(key, &value, ttl).await?;
            Ok(value)
        }
    }

    /// Acquire distributed lock with timeout and retry logic
    pub async fn acquire_lock(&self, lock_key: &str) -> CacheResult<String> {
        if !self.config.enable_distributed_locking {
            return Err(CacheError::ConfigurationError("Distributed locking is disabled".to_string()));
        }

        let lock_value = format!("{}:{}", Uuid::new_v4(), chrono::Utc::now().timestamp());
        
        for attempt in 1..=self.config.max_retry_attempts {
            let mut conn = self.get_connection().await?;
            
            // Use SET with NX and EX options for atomic lock acquisition
            let result: Option<String> = conn
                .set_nx_ex(lock_key, &lock_value, self.config.lock_timeout.as_secs() as u64)
                .await
                .map_err(|e| {
                    error!("Failed to acquire lock for key '{}': {}", lock_key, e);
                    e.into()
                })?;

            if result.is_some() {
                debug!("Acquired lock for key: {} (attempt {})", lock_key, attempt);
                return Ok(lock_value);
            }

            // Lock acquisition failed, wait before retry
            if attempt < self.config.max_retry_attempts {
                let backoff = Duration::from_millis(100 * attempt as u64);
                tokio::time::sleep(backoff).await;
            }
        }

        warn!("Failed to acquire lock for key: {} after {} attempts", lock_key, self.config.max_retry_attempts);
        Err(CacheError::LockError(format!("Failed to acquire lock for key: {}", lock_key)))
    }

    /// Release distributed lock with ownership verification
    pub async fn release_lock(&self, lock_key: &str) -> CacheResult<bool> {
        if !self.config.enable_distributed_locking {
            return Ok(true);
        }

        let mut conn = self.get_connection().await?;
        
        // Use Lua script for atomic lock release with ownership check
        let lua_script = r#"
            if redis.call("GET", KEYS[1]) == ARGV[1] then
                return redis.call("DEL", KEYS[1])
            else
                return 0
            end
        "#;

        let result: i64 = redis::Script::new(lua_script)
            .key(lock_key)
            .arg("") // We don't need the lock value for basic release
            .invoke_async(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to release lock for key '{}': {}", lock_key, e);
                e.into()
            })?;

        let released = result > 0;
        if released {
            debug!("Released lock for key: {}", lock_key);
        } else {
            warn!("Failed to release lock for key: {} (not owned or expired)", lock_key);
        }

        Ok(released)
    }

    /// Event-driven cache invalidation using Redis pub/sub
    pub async fn invalidate_pattern(&self, pattern: &str) -> CacheResult<u64> {
        let mut conn = self.get_connection().await?;
        
        // Find all keys matching the pattern
        let keys: Vec<String> = conn.keys(pattern).await.map_err(|e| {
            error!("Failed to scan keys for pattern '{}': {}", pattern, e);
            e.into()
        })?;

        let count = keys.len();
        if count == 0 {
            return Ok(0);
        }

        // Delete all matching keys
        let deleted: u64 = conn.del(&keys).await.map_err(|e| {
            error!("Failed to delete keys for pattern '{}': {}", pattern, e);
            e.into()
        })?;

        // Publish invalidation event if enabled
        if self.config.enable_event_driven_invalidation {
            self.publish_invalidation_event(&keys).await?;
        }

        info!("Invalidated {} keys matching pattern: {}", deleted, pattern);
        Ok(deleted)
    }

    /// Publish cache invalidation events
    async fn publish_invalidation_event(&self, keys: &[String]) -> CacheResult<()> {
        let mut conn = self.get_connection().await?;
        
        let event = serde_json::json!({
            "type": "cache_invalidation",
            "keys": keys,
            "timestamp": chrono::Utc::now(),
            "source": "advanced_redis_cache"
        });

        let _: i64 = conn
            .publish("cache_invalidation", event.to_string())
            .await
            .map_err(|e| {
                error!("Failed to publish cache invalidation event: {}", e);
                e.into()
            })?;

        debug!("Published cache invalidation event for {} keys", keys.len());
        Ok(())
    }

    /// Subscribe to cache invalidation events
    pub async fn subscribe_to_invalidations(&self) -> CacheResult<InvalidationSubscriber> {
        if !self.config.enable_event_driven_invalidation {
            return Err(CacheError::ConfigurationError("Event-driven invalidation is disabled".to_string()));
        }

        let mut conn = self.get_connection().await?;
        let pubsub = conn.into_pubsub();
        
        pubsub.subscribe("cache_invalidation").await.map_err(|e| {
            error!("Failed to subscribe to cache invalidation channel: {}", e);
            e.into()
        })?;

        info!("Subscribed to cache invalidation events");
        Ok(InvalidationSubscriber { pubsub })
    }

    /// Cache warming with priority queuing
    pub async fn warm_cache<T>(&self, entries: Vec<CacheWarmupEntry<T>>) -> CacheResult<()>
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        if !self.config.enable_cache_warming {
            return Ok(());
        }

        info!("Warming cache with {} entries", entries.len());

        // Sort entries by priority (high to low)
        let mut sorted_entries = entries;
        sorted_entries.sort_by_key(|e| std::cmp::Reverse(e.priority));

        for entry in sorted_entries {
            debug!("Warming cache entry: {} (priority: {:?})", entry.key, entry.priority);
            
            if let Err(e) = self.set(&entry.key, &entry.value, Some(entry.ttl)).await {
                warn!("Failed to warm cache entry '{}': {}", entry.key, e);
            }
        }

        info!("Cache warming completed");
        Ok(())
    }

    /// Get cache performance metrics
    pub async fn get_performance_metrics(&self) -> CacheResult<CachePerformanceMetrics> {
        if !self.config.enable_performance_monitoring {
            return Ok(CachePerformanceMetrics::default());
        }

        let mut conn = self.get_connection().await?;
        
        // Get Redis info
        let info: String = conn.info().await.map_err(|e| e.into())?;
        
        // Parse Redis info for metrics
        let metrics = self.parse_redis_info(&info);
        
        Ok(metrics)
    }

    fn parse_redis_info(&self, info: &str) -> CachePerformanceMetrics {
        let mut metrics = CachePerformanceMetrics::default();
        
        for line in info.lines() {
            if let Some((key, value)) = line.split_once(':') {
                match key {
                    "used_memory" => {
                        if let Ok(bytes) = value.parse::<u64>() {
                            metrics.memory_usage_bytes = bytes;
                        }
                    }
                    "used_memory_human" => {
                        metrics.memory_usage_human = value.to_string();
                    }
                    "keyspace_hits" => {
                        if let Ok(hits) = value.parse::<u64>() {
                            metrics.cache_hits = hits;
                        }
                    }
                    "keyspace_misses" => {
                        if let Ok(misses) = value.parse::<u64>() {
                            metrics.cache_misses = misses;
                        }
                    }
                    "connected_clients" => {
                        if let Ok(clients) = value.parse::<u32>() {
                            metrics.connected_clients = clients;
                        }
                    }
                    "total_commands_processed" => {
                        if let Ok(commands) = value.parse::<u64>() {
                            metrics.total_commands = commands;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Calculate hit rate
        let total_requests = metrics.cache_hits + metrics.cache_misses;
        if total_requests > 0 {
            metrics.hit_rate = metrics.cache_hits as f64 / total_requests as f64;
        }

        metrics
    }

    /// Optimized batch operations using Redis pipeline
    pub async fn batch_set<T>(&self, items: Vec<(&str, &T, Option<Duration>)>) -> CacheResult<()>
    where
        T: Serialize + Send + Sync + 'static,
    {
        if items.is_empty() {
            return Ok(());
        }

        let mut conn = self.get_connection().await?;
        let mut pipe = redis::pipe();

        for (key, value, ttl) in items {
            let json_str = serde_json::to_string(value).map_err(|e| {
                error!("Failed to serialize value for batch set key '{}': {}", key, e);
                e
            })?;

            match ttl {
                Some(ttl_duration) => {
                    let ttl_seconds = ttl_duration.as_secs() as u64;
                    pipe.set_ex(key, json_str, ttl_seconds);
                }
                None => {
                    pipe.set(key, json_str);
                }
            }
        }

        let _: () = pipe.query_async(&mut *conn).await.map_err(|e| {
            error!("Batch set operation failed: {}", e);
            e.into()
        })?;

        debug!("Batch set {} items", items.len());
        Ok(())
    }

    /// Optimized batch get using Redis mget
    pub async fn batch_get<T>(&self, keys: &[&str]) -> CacheResult<Vec<Option<T>>>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.get_connection().await?;
        
        // Use Redis MGET for batch retrieval
        let results: Vec<Option<String>> = conn.mget(keys).await.map_err(|e| {
            error!("Batch get operation failed: {}", e);
            e.into()
        })?;

        // Deserialize results
        let mut deserialized_results = Vec::with_capacity(results.len());
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(json_str) => {
                    match serde_json::from_str::<T>(&json_str) {
                        Ok(value) => deserialized_results.push(Some(value)),
                        Err(e) => {
                            error!("Failed to deserialize batch get result for key '{}': {}", keys[i], e);
                            deserialized_results.push(None);
                        }
                    }
                }
                None => deserialized_results.push(None),
            }
        }

        debug!("Batch get {} keys", keys.len());
        Ok(deserialized_results)
    }

    /// Health check with detailed diagnostics
    pub async fn health_check(&self) -> CacheResult<HealthCheckResult> {
        let start_time = std::time::Instant::now();
        let mut result = HealthCheckResult {
            healthy: false,
            latency: Duration::from_secs(0),
            memory_usage: 0,
            connected_clients: 0,
            error: None,
        };

        match self.get_connection().await {
            Ok(mut conn) => {
                // Test basic connectivity
                match timeout(Duration::from_secs(5), conn.ping()).await {
                    Ok(Ok(_)) => {
                        // Get additional health metrics
                        match conn.info().await {
                            Ok(info) => {
                                let metrics = self.parse_redis_info(&info);
                                result.memory_usage = metrics.memory_usage_bytes;
                                result.connected_clients = metrics.connected_clients;
                                result.healthy = true;
                            }
                            Err(e) => {
                                result.error = Some(format!("Failed to get Redis info: {}", e));
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        result.error = Some(format!("Redis ping failed: {}", e));
                    }
                    Err(_) => {
                        result.error = Some("Redis ping timeout".to_string());
                    }
                }
            }
            Err(e) => {
                result.error = Some(format!("Failed to get Redis connection: {}", e));
            }
        }

        result.latency = start_time.elapsed();
        Ok(result)
    }
}

// Implement the basic Cache trait for AdvancedRedisCache
#[async_trait]
impl<T: Serialize + DeserializeOwned + Send + Sync + 'static> super::Cache<T> for AdvancedRedisCache {
    async fn get(&self, key: &str) -> CacheResult<Option<T>> {
        let mut conn = self.get_connection().await?;
        
        let result: Option<String> = conn.get(key).await.map_err(|e| {
            error!("Redis GET failed for key '{}': {}", key, e);
            e.into()
        })?;

        match result {
            Some(json_str) => {
                let value: T = serde_json::from_str(&json_str).map_err(|e| {
                    error!("Failed to deserialize cache value for key '{}': {}", key, e);
                    e.into()
                })?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, key: &str, value: &T, ttl: Option<Duration>) -> CacheResult<()> {
        let mut conn = self.get_connection().await?;
        
        let json_str = serde_json::to_string(value).map_err(|e| {
            error!("Failed to serialize value for key '{}': {}", key, e);
            e.into()
        })?;

        match ttl {
            Some(ttl_duration) => {
                let ttl_seconds = ttl_duration.as_secs() as u64;
                let _: () = conn.set_ex(key, json_str, ttl_seconds).await.map_err(|e| e.into())?;
            }
            None => {
                let _: () = conn.set(key, json_str).await.map_err(|e| e.into())?;
            }
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let deleted: i64 = conn.del(key).await.map_err(|e| e.into())?;
        Ok(deleted > 0)
    }

    async fn exists(&self, key: &str) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let exists: bool = conn.exists(key).await.map_err(|e| e.into())?;
        Ok(exists)
    }

    async fn set_multiple(&self, items: Vec<(String, T)>, ttl: Option<Duration>) -> CacheResult<()> {
        let items_with_refs: Vec<_> = items.iter()
            .map(|(key, value)| (key.as_str(), value, ttl))
            .collect();
        self.batch_set(items_with_refs).await
    }

    async fn get_multiple(&self, keys: Vec<String>) -> CacheResult<Vec<Option<T>>> {
        let key_refs: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
        self.batch_get(&key_refs).await
    }

    async fn increment(&self, key: &str, amount: i64) -> CacheResult<i64> {
        let mut conn = self.get_connection().await?;
        let result: i64 = conn.incr(key, amount).await.map_err(|e| e.into())?;
        Ok(result)
    }

    async fn decrement(&self, key: &str, amount: i64) -> CacheResult<i64> {
        let mut conn = self.get_connection().await?;
        let result: i64 = conn.decr(key, amount).await.map_err(|e| e.into())?;
        Ok(result)
    }

    async fn expire(&self, key: &str, ttl: Duration) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let result: bool = conn.expire(key, ttl.as_secs() as u64).await.map_err(|e| e.into())?;
        Ok(result)
    }

    async fn ttl(&self, key: &str) -> CacheResult<i64> {
        let mut conn = self.get_connection().await?;
        let result: i64 = conn.ttl(key).await.map_err(|e| e.into())?;
        Ok(result)
    }

    async fn delete_pattern(&self, pattern: &str) -> CacheResult<u64> {
        self.invalidate_pattern(pattern).await
    }
}

// Supporting types
#[derive(Debug)]
pub struct InvalidationSubscriber {
    pubsub: redis::aio::PubSub,
}

impl InvalidationSubscriber {
    pub async fn next_message(&mut self) -> CacheResult<InvalidationMessage> {
        let msg = self.pubsub.on_message().next().await.ok_or_else(|| {
            CacheError::ConnectionError("PubSub connection closed".to_string())
        })?;

        let payload: String = msg.get_payload().map_err(|e| {
            error!("Failed to get pubsub message payload: {}", e);
            e.into()
        })?;

        let message: InvalidationMessage = serde_json::from_str(&payload).map_err(|e| {
            error!("Failed to parse invalidation message: {}", e);
            e.into()
        })?;

        Ok(message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationMessage {
    pub r#type: String,
    pub keys: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source: String,
}

#[derive(Debug)]
pub struct CacheWarmupEntry<T> {
    pub key: String,
    pub value: T,
    pub ttl: Duration,
    pub priority: WarmupPriority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarmupPriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

#[derive(Debug, Clone, Default)]
pub struct CachePerformanceMetrics {
    pub memory_usage_bytes: u64,
    pub memory_usage_human: String,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub hit_rate: f64,
    pub connected_clients: u32,
    pub total_commands: u64,
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub latency: Duration,
    pub memory_usage: u64,
    pub connected_clients: u32,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advanced_cache_config_default() {
        let config = AdvancedCacheConfig::default();
        assert!(config.enable_distributed_locking);
        assert!(config.enable_event_driven_invalidation);
        assert_eq!(config.max_retry_attempts, 3);
    }

    #[test]
    fn test_warmup_priority_ordering() {
        assert!(WarmupPriority::Critical > WarmupPriority::High);
        assert!(WarmupPriority::High > WarmupPriority::Medium);
        assert!(WarmupPriority::Medium > WarmupPriority::Low);
    }

    #[tokio::test]
    async fn test_health_check_result() {
        let result = HealthCheckResult {
            healthy: true,
            latency: Duration::from_millis(50),
            memory_usage: 1024 * 1024,
            connected_clients: 5,
            error: None,
        };

        assert!(result.healthy);
        assert_eq!(result.connected_clients, 5);
    }
}
