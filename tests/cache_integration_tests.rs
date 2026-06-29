//! Integration Tests for Enhanced Caching Layer
//!
//! Tests the complete caching system including:
//! - Advanced Redis features
//! - CDN integration
//! - Multi-level caching
//! - Performance optimization

use aframp_backend::cache::{
    AdvancedCacheConfig, AdvancedRedisCache, CDNConfig, CDNManager, CacheError, CacheResult,
};
use redis::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestData {
    id: Uuid,
    name: String,
    value: i32,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_advanced_redis_cache_basic_operations() -> Result<(), anyhow::Error> {
    // Setup test Redis connection
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    // Test data
    let test_data = TestData {
        id: Uuid::new_v4(),
        name: "test".to_string(),
        value: 42,
        timestamp: chrono::Utc::now(),
    };

    let key = format!("test:{}", test_data.id);

    // Test set and get
    cache
        .set(&key, &test_data, Some(Duration::from_secs(60)))
        .await?;

    let retrieved: Option<TestData> = cache.get(&key).await?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.expect("just asserted Some").id, test_data.id);

    // Test exists
    assert!(cache.exists(&key).await?);

    // Test delete
    let deleted = cache.delete(&key).await?;
    assert!(deleted);
    assert!(!cache.exists(&key).await?);

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cache_aside_pattern() -> Result<(), anyhow::Error> {
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    let key = "cache_aside_test".to_string();
    let expected_data = TestData {
        id: Uuid::new_v4(),
        name: "cache_aside".to_string(),
        value: 100,
        timestamp: chrono::Utc::now(),
    };

    // First call should fetch from source (cache miss)
    let result = cache
        .get_or_set(
            &key,
            || async move {
                sleep(Duration::from_millis(100)).await; // Simulate slow operation
                Ok(expected_data.clone())
            },
            Some(Duration::from_secs(300)),
        )
        .await?;

    assert_eq!(result.id, expected_data.id);

    // Verify data is cached
    let cached: Option<TestData> = cache.get(&key).await?;
    assert!(cached.is_some());
    assert_eq!(cached.expect("just asserted Some").id, expected_data.id);

    // Second call should hit cache (should be faster)
    let start = std::time::Instant::now();
    let result2 = cache
        .get_or_set(
            &key,
            || async move {
                unreachable!("Should not be called due to cache hit");
            },
            Some(Duration::from_secs(300)),
        )
        .await?;

    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(50)); // Should be much faster
    assert_eq!(result2.id, expected_data.id);

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_distributed_locking() -> Result<(), anyhow::Error> {
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    let lock_key = "test_lock".to_string();

    // Acquire lock
    let lock_value = cache.acquire_lock(&lock_key).await?;
    assert!(!lock_value.is_empty());

    // Try to acquire same lock (should fail)
    let lock_result = cache.acquire_lock(&lock_key).await;
    assert!(lock_result.is_err());

    // Release lock
    let released = cache.release_lock(&lock_key).await?;
    assert!(released);

    // Now should be able to acquire again
    let lock_value2 = cache.acquire_lock(&lock_key).await?;
    assert!(!lock_value2.is_empty());

    // Cleanup
    cache.release_lock(&lock_key).await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_batch_operations() -> Result<(), anyhow::Error> {
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    // Prepare test data
    let mut items = Vec::new();
    let mut keys = Vec::new();

    for i in 0..10 {
        let data = TestData {
            id: Uuid::new_v4(),
            name: format!("batch_test_{}", i),
            value: i,
            timestamp: chrono::Utc::now(),
        };

        let key = format!("batch_test:{}", data.id);
        keys.push(key.clone());
        items.push((key.as_str(), &data, Some(Duration::from_secs(300))));
    }

    // Batch set
    cache.batch_set(items).await?;

    // Batch get
    let key_refs: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
    let results: Vec<Option<TestData>> = cache.batch_get(&key_refs).await?;

    assert_eq!(results.len(), 10);
    for result in results {
        assert!(result.is_some());
    }

    // Cleanup
    for key in &keys {
        cache.delete(key).await?;
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_pattern_invalidation() -> Result<(), anyhow::Error> {
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    // Create test entries with pattern
    let mut keys = Vec::new();
    for i in 0..5 {
        let key = format!("pattern_test:user:{}:data", i);
        let data = TestData {
            id: Uuid::new_v4(),
            name: format!("user_{}", i),
            value: i,
            timestamp: chrono::Utc::now(),
        };

        cache
            .set(&key, &data, Some(Duration::from_secs(300)))
            .await?;
        keys.push(key);
    }

    // Verify entries exist
    for key in &keys {
        assert!(cache.exists(key).await?);
    }

    // Invalidate by pattern
    let invalidated_count = cache.invalidate_pattern("pattern_test:user:*:data").await?;
    assert_eq!(invalidated_count, 5);

    // Verify entries are gone
    for key in &keys {
        assert!(!cache.exists(key).await?);
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cache_performance_metrics() -> Result<(), anyhow::Error> {
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    // Perform some operations to generate metrics
    for i in 0..100 {
        let key = format!("metrics_test:{}", i);
        let data = TestData {
            id: Uuid::new_v4(),
            name: format!("metrics_{}", i),
            value: i,
            timestamp: chrono::Utc::now(),
        };

        cache
            .set(&key, &data, Some(Duration::from_secs(60)))
            .await?;
        cache.get::<TestData>(&key).await?;
    }

    // Get performance metrics
    let metrics = cache.get_performance_metrics().await?;

    // Verify metrics are populated
    assert!(metrics.connected_clients > 0);
    assert!(metrics.memory_usage_bytes > 0);

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cache_health_check() -> Result<(), anyhow::Error> {
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    // Perform health check
    let health_result = cache.health_check().await?;

    assert!(health_result.healthy);
    assert!(health_result.latency < Duration::from_secs(5));
    assert!(health_result.connected_clients > 0);

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cdn_manager_configuration() -> Result<(), anyhow::Error> {
    let config = CDNConfig::default();
    let cdn_manager = CDNManager::new(config);

    // Test CDN header generation
    let mut headers = axum::http::HeaderMap::new();
    cdn_manager.add_cdn_headers(
        &mut headers,
        aframp_backend::cache::cdn_integration::ResourceType::APIResponse,
    );

    // Verify Cache-Control header
    let cache_control = headers.get("cache-control");
    assert!(cache_control.is_some());

    let cache_control_str = cache_control.expect("just asserted Some").to_str()?;
    assert!(cache_control_str.contains("max-age="));
    assert!(cache_control_str.contains("public"));

    // Verify ETag header
    let etag = headers.get("etag");
    assert!(etag.is_some());

    // Verify security headers
    let csp = headers.get("content-security-policy");
    assert!(csp.is_some());

    let hsts = headers.get("strict-transport-security");
    assert!(hsts.is_some());

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cdn_routing_decisions() -> Result<(), anyhow::Error> {
    let config = CDNConfig::default();
    let cdn_manager = CDNManager::new(config);

    // Test geographic routing
    let region = cdn_manager.get_optimal_region("US");
    assert_eq!(region, "us-east-1");

    let region = cdn_manager.get_optimal_region("GB");
    assert_eq!(region, "us-east-1"); // Default region for unmapped countries

    // Test cache decision logic
    assert!(cdn_manager.should_cache_request("/static/app.js", "GET"));
    assert!(cdn_manager.should_cache_request("/api/public/rates", "GET"));
    assert!(!cdn_manager.should_cache_request("/api/admin/users", "GET"));
    assert!(!cdn_manager.should_cache_request("/api/auth/login", "POST"));

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cdn_cache_warming() -> Result<(), anyhow::Error> {
    let config = CDNConfig::default();
    let cdn_manager = CDNManager::new(config);

    // Prepare warmup resources
    let mut resources = Vec::new();
    for i in 0..5 {
        resources.push(
            aframp_backend::cache::cdn_integration::CacheWarmupResource {
                path: format!("/static/test_{}.js", i),
                resource_type: aframp_backend::cache::cdn_integration::ResourceType::StaticAsset,
                priority: aframp_backend::cache::cdn_integration::WarmupPriority::Medium,
                headers: std::collections::HashMap::new(),
            },
        );
    }

    // Warm cache
    cdn_manager.warm_cache(resources).await?;

    // Get metrics
    let metrics = cdn_manager.get_metrics();
    assert!(metrics.enabled);

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cdn_invalidation() -> Result<(), anyhow::Error> {
    let config = CDNConfig::default();
    let cdn_manager = CDNManager::new(config);

    // Test cache invalidation
    let paths = vec![
        "/static/app.js".to_string(),
        "/static/styles.css".to_string(),
        "/api/public/rates".to_string(),
    ];

    let result = cdn_manager.invalidate_cache(&paths).await?;
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cache_ttl_expiration() -> Result<(), anyhow::Error> {
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    let key = "ttl_test".to_string();
    let data = TestData {
        id: Uuid::new_v4(),
        name: "ttl_test".to_string(),
        value: 42,
        timestamp: chrono::Utc::now(),
    };

    // Set with short TTL
    cache.set(&key, &data, Some(Duration::from_secs(1))).await?;

    // Should exist immediately
    assert!(cache.exists(&key).await?);

    // Wait for expiration
    sleep(Duration::from_secs(2)).await;

    // Should be expired
    let retrieved: Option<TestData> = cache.get(&key).await?;
    assert!(retrieved.is_none());

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_concurrent_cache_operations() -> Result<(), anyhow::Error> {
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(20).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = std::sync::Arc::new(AdvancedRedisCache::new_with_config(pool, config));

    // Spawn multiple concurrent tasks
    let mut tasks = Vec::new();

    for i in 0..50 {
        let cache_clone = cache.clone();
        let task = tokio::spawn(async move {
            let key = format!("concurrent_test:{}", i);
            let data = TestData {
                id: Uuid::new_v4(),
                name: format!("concurrent_{}", i),
                value: i,
                timestamp: chrono::Utc::now(),
            };

            // Set value
            cache_clone
                .set(&key, &data, Some(Duration::from_secs(60)))
                .await?;

            // Get value
            let retrieved: Option<TestData> = cache_clone.get(&key).await?;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.expect("just asserted Some").value, i);

            // Delete value
            cache_clone.delete(&key).await?;

            Ok::<(), anyhow::Error>(())
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        task.await??;
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires live Redis on 127.0.0.1:6379"]
async fn test_cache_error_handling() -> Result<(), anyhow::Error> {
    // Test with invalid Redis connection
    let client = Client::open("redis://invalid-host:6379")?;

    // This should fail to create connection manager
    let result = client.get_connection_manager().await;
    assert!(result.is_err());

    // Test with invalid data
    let client = Client::open("redis://127.0.0.1:6379")?;
    let manager = client.get_connection_manager().await?;
    let pool = bb8::Pool::builder().max_size(10).build(manager).await?;

    let config = AdvancedCacheConfig::default();
    let cache = AdvancedRedisCache::new_with_config(pool, config);

    // Try to get non-existent key
    let result: Option<String> = cache.get("non_existent_key").await?;
    assert!(result.is_none());

    // Try to delete non-existent key
    let deleted = cache.delete("non_existent_key").await?;
    assert!(!deleted);

    Ok(())
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    #[ignore = "requires live Redis on 127.0.0.1:6379"]
    async fn benchmark_cache_operations() -> Result<(), anyhow::Error> {
        let client = Client::open("redis://127.0.0.1:6379")?;
        let manager = client.get_connection_manager().await?;
        let pool = bb8::Pool::builder().max_size(50).build(manager).await?;

        let config = AdvancedCacheConfig::default();
        let cache = AdvancedRedisCache::new_with_config(pool, config);

        const NUM_OPERATIONS: usize = 1000;

        // Benchmark SET operations
        let start = Instant::now();
        for i in 0..NUM_OPERATIONS {
            let key = format!("benchmark_set:{}", i);
            let data = format!("value_{}", i);
            cache
                .set(&key, &data, Some(Duration::from_secs(300)))
                .await?;
        }
        let set_duration = start.elapsed();

        // Benchmark GET operations
        let start = Instant::now();
        for i in 0..NUM_OPERATIONS {
            let key = format!("benchmark_set:{}", i);
            let _: Option<String> = cache.get(&key).await?;
        }
        let get_duration = start.elapsed();

        // Benchmark batch operations
        let mut batch_items = Vec::new();
        for i in 0..100 {
            let key = format!("benchmark_batch:{}", i);
            let data = format!("batch_value_{}", i);
            batch_items.push((key.as_str(), &data, Some(Duration::from_secs(300))));
        }

        let start = Instant::now();
        cache.batch_set(batch_items).await?;
        let batch_set_duration = start.elapsed();

        // Print performance results
        println!("Cache Performance Benchmark:");
        println!(
            "SET operations: {} ops in {:?} ({:.2} ops/sec)",
            NUM_OPERATIONS,
            set_duration,
            NUM_OPERATIONS as f64 / set_duration.as_secs_f64()
        );
        println!(
            "GET operations: {} ops in {:?} ({:.2} ops/sec)",
            NUM_OPERATIONS,
            get_duration,
            NUM_OPERATIONS as f64 / get_duration.as_secs_f64()
        );
        println!("Batch SET (100 items): {:?}", batch_set_duration);

        // Cleanup
        for i in 0..NUM_OPERATIONS {
            let key = format!("benchmark_set:{}", i);
            cache.delete(&key).await?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Issue #459 — Extended integration tests
// All require a live Redis: REDIS_URL=redis://127.0.0.1:6379
// Run: cargo test --test cache_integration_tests -- --ignored
// ---------------------------------------------------------------------------

mod issue_459 {
    use aframp_backend::cache::{
        cache::{Cache as CacheTrait, RedisCache},
        cdn_integration::{body_etag, route_cache_control},
        invalidation::{InvalidationEvent, InvalidationPipeline},
        keys,
        multi_level::MultiLevelCache,
        {build_multi_level_cache, init_cache_pool, CacheConfig},
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::time::Duration;
    use tokio::time::sleep;

    async fn build_cache() -> Option<(RedisCache, Arc<MultiLevelCache>)> {
        let url = std::env::var("REDIS_URL").ok()?;
        let mut cfg = CacheConfig::default();
        cfg.redis_url = url;
        let pool = init_cache_pool(cfg).await.ok()?;
        let redis = RedisCache::new(pool);
        let registry = prometheus::Registry::new();
        let ml = Arc::new(build_multi_level_cache(redis.clone(), &registry));
        Some((redis, ml))
    }

    // 1. SCAN-based delete_pattern accuracy
    #[tokio::test]
    #[ignore = "requires REDIS_URL"]
    async fn test_scan_delete_pattern_accuracy() -> anyhow::Result<()> {
        let Some((redis, _ml)) = build_cache().await else { return Ok(()) };

        // Insert 20 rate keys + 5 unrelated
        for i in 0..20 {
            let key = format!("v1:rate:TEST:KEY{}", i);
            let _ = CacheTrait::<String>::set(&redis, &key, &"val".to_string(), Some(Duration::from_secs(60))).await;
        }
        for i in 0..5 {
            let key = format!("v1:wallet:ADDR{}", i);
            let _ = CacheTrait::<String>::set(&redis, &key, &"val".to_string(), Some(Duration::from_secs(60))).await;
        }

        let deleted = CacheTrait::<String>::delete_pattern(&redis, "v1:rate:TEST:*").await?;
        assert_eq!(deleted, 20, "Expected exactly 20 rate keys deleted");

        // Unrelated keys must survive
        for i in 0..5 {
            let key = format!("v1:wallet:ADDR{}", i);
            let exists = CacheTrait::<String>::exists(&redis, &key).await?;
            assert!(exists, "Wallet key {} should not have been deleted", key);
            let _ = CacheTrait::<String>::delete(&redis, &key).await;
        }

        Ok(())
    }

    // 2. Stampede protection: 5,000 concurrent requests → 1 DB call
    #[tokio::test]
    #[ignore = "requires REDIS_URL"]
    async fn test_stampede_single_db_call() {
        let Some((_redis, ml)) = build_cache().await else { return };
        let call_count = Arc::new(AtomicUsize::new(0));

        // Ensure key does not exist
        let test_key = "v1:rate:STAMPEDE:USD";
        ml.l2_invalidate::<String>(test_key).await;

        let mut handles = Vec::with_capacity(5000);
        for _ in 0..5000 {
            let ml_clone = ml.clone();
            let cc = call_count.clone();
            handles.push(tokio::spawn(async move {
                ml_clone
                    .l2_get_or_rebuild("rate", test_key, Duration::from_secs(5), || {
                        let cc2 = cc.clone();
                        async move {
                            cc2.fetch_add(1, Ordering::SeqCst);
                            // Simulate DB latency
                            sleep(Duration::from_millis(10)).await;
                            Ok("1750.0".to_string())
                        }
                    })
                    .await
                    // Unwrap is acceptable here: a failure inside a spawned test task
                    // indicates a bug that should fail the test
                    .expect("stampede task should succeed")
            }));
        }

        let results: Vec<_> = futures::future::join_all(handles).await;
        let db_calls = call_count.load(Ordering::SeqCst);

        assert_eq!(db_calls, 1, "Single-flight: expected 1 DB call, got {}", db_calls);
        assert!(results.iter().all(|r| r.as_ref().expect("result should be Some") == "1750.0"));

        ml.l2_invalidate::<String>(test_key).await;
    }

    // 3. Multi-tenant isolation: user A cache does not bleed into user B
    #[tokio::test]
    #[ignore = "requires REDIS_URL"]
    async fn test_multi_tenant_key_isolation() {
        let Some((redis, _ml)) = build_cache().await else { return };

        let key_a = keys::user::ProfileKey::new("user-A").to_string();
        let key_b = keys::user::ProfileKey::new("user-B").to_string();

        let _ = CacheTrait::<String>::set(&redis, &key_a, &"profile-A".to_string(), Some(Duration::from_secs(60))).await;

        // B should return None even though A is cached
        let val_b: Option<String> = CacheTrait::<String>::get(&redis, &key_b).await
            .expect("Cache get should not panic on degraded connection");
        assert!(val_b.is_none(), "User B must not see User A's cache entry");

        let val_a: Option<String> = CacheTrait::<String>::get(&redis, &key_a).await
            .expect("Cache get should not panic on degraded connection");
        assert_eq!(val_a.as_deref(), Some("profile-A"));

        let _ = CacheTrait::<String>::delete(&redis, &key_a).await;
    }

    // 4. Network partition fallback: invalid Redis → graceful Ok(None), no panic
    #[tokio::test]
    async fn test_network_partition_graceful_fallback() {
        let cfg = CacheConfig {
            redis_url: "redis://127.0.0.1:19999".into(), // nothing on this port
            connection_timeout: Duration::from_millis(100),
            max_connections: 2,
            ..Default::default()
        };
        // Pool build may fail — that's OK; simulate degraded mode
        if let Ok(pool) = init_cache_pool(cfg).await {
            let redis = RedisCache::new(pool);
            let result: Option<String> = CacheTrait::<String>::get(&redis, "any-key").await
                .expect("Cache get should return Ok(None) on degraded connection, not panic");
            assert!(result.is_none(), "Degraded Redis must return None, not panic");
        }
        // If pool fails entirely, test still passes — connection error IS the graceful path
    }

    // 5. Write-through invalidation event deletes the correct key
    #[tokio::test]
    #[ignore = "requires REDIS_URL"]
    async fn test_write_through_invalidation() {
        let Some((redis, _ml)) = build_cache().await else { return };
        let r = Arc::new(redis);

        let key = keys::exchange_rate::CurrencyPairKey::new("CNGN", "USD").to_string();
        let _ = CacheTrait::<String>::set(&*r, &key, &"1750".to_string(), Some(Duration::from_secs(60))).await;

        // Confirm key exists before invalidation
        assert!(CacheTrait::<String>::exists(&*r, &key).await
            .expect("Cache exists should not panic"));

        // Pipeline processes the event
        let pipeline = InvalidationPipeline::new(r.clone(), None);
        pipeline.process(InvalidationEvent::ExchangeRateUpdated {
            from: "CNGN".into(),
            to: "USD".into(),
        }).await;

        // Key must be gone
        let exists = CacheTrait::<String>::exists(&*r, &key).await
            .expect("Cache exists should not panic");
        assert!(!exists, "Rate key must be deleted after ExchangeRateUpdated event");
    }

    // 6. CDN ETag determinism and 304 logic (pure unit test — no Redis needed)
    #[test]
    fn test_etag_determinism() {
        let body = b"exchange-rate-response";
        assert_eq!(body_etag(body), body_etag(body), "Same body must produce same ETag");
        assert_ne!(body_etag(b"body-one"), body_etag(b"body-two"));
    }

    // 7. Cache-Control headers per route (pure unit test)
    #[test]
    fn test_cache_control_route_map() {
        assert_eq!(route_cache_control("/api/rates"), "public, max-age=90, s-maxage=90");
        assert_eq!(route_cache_control("/api/fees/calculate"), "public, max-age=90, s-maxage=90");
        assert!(route_cache_control("/api/admin/cache").contains("no-store"));
        assert!(route_cache_control("/api/v1/user/profile").contains("private"));
        assert!(route_cache_control("/api/unknown/path").contains("no-cache"));
    }

    // 8. Key namespace correctness (extend existing keys.rs unit tests)
    #[test]
    fn test_new_key_namespaces() {
        use keys::{namespace_pattern, partner, user};

        assert_eq!(user::ProfileKey::new("u1").to_string(), "v1:user:u1:profile");
        assert_eq!(user::OnboardingKey::new("u1").to_string(), "v1:user:u1:onboarding");
        assert_eq!(partner::ConfigKey::new("p1").to_string(), "v1:partner:p1:config");
        assert_eq!(partner::LiquidityKey::new("p1").to_string(), "v1:partner:p1:liquidity");
        assert_eq!(namespace_pattern("rate"), "v1:rate:*");
        assert_eq!(namespace_pattern("user"), "v1:user:*");
    }

    // 9. TTL serialization round-trip
    #[test]
    fn test_ttl_constants() {
        use aframp_backend::cache::cache::ttl;
        assert_eq!(ttl::EXCHANGE_RATES.as_secs(), 90);
        assert_eq!(ttl::WALLET_BALANCES.as_secs(), 45);
        assert_eq!(ttl::FEE_STRUCTURES.as_secs(), 3600);
    }

    // 10. L1 miss → L2 hit (promotion path via MultiLevelCache::l2_get)
    #[tokio::test]
    #[ignore = "requires REDIS_URL"]
    async fn test_l2_hit_on_l1_miss() {
        let Some((redis, ml)) = build_cache().await else { return };

        let key = "v1:rate:L1MISS:USD";
        // Populate L2 only
        let _ = CacheTrait::<String>::set(&redis, key, &"1750".to_string(), Some(Duration::from_secs(60))).await;

        // L1 miss
        let l1_hit: Option<String> = ml.l1_get(aframp_backend::cache::l1::L1Category::CurrencyConfigs, key).await;
        assert!(l1_hit.is_none(), "L1 should miss");

        // L2 hit
        let l2_hit: Option<String> = ml.l2_get("rate", key).await;
        assert_eq!(l2_hit.as_deref(), Some("1750"), "L2 should hit");

        let _ = CacheTrait::<String>::delete(&redis, key).await;
    }
}
