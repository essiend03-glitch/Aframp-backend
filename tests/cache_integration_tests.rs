//! Integration Tests for Enhanced Caching Layer
//!
//! Tests the complete caching system including:
//! - Advanced Redis features
//! - CDN integration
//! - Multi-level caching
//! - Performance optimization

use aframp_backend::cache::{
    AdvancedRedisCache, AdvancedCacheConfig, CDNManager, CDNConfig, 
    CacheError, CacheResult
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
    cache.set(&key, &test_data, Some(Duration::from_secs(60))).await?;
    
    let retrieved: Option<TestData> = cache.get(&key).await?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, test_data.id);

    // Test exists
    assert!(cache.exists(&key).await?);

    // Test delete
    let deleted = cache.delete(&key).await?;
    assert!(deleted);
    assert!(!cache.exists(&key).await?);

    Ok(())
}

#[tokio::test]
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
    let result = cache.get_or_set(&key, || async move {
        sleep(Duration::from_millis(100)).await; // Simulate slow operation
        Ok(expected_data.clone())
    }, Some(Duration::from_secs(300))).await?;

    assert_eq!(result.id, expected_data.id);

    // Verify data is cached
    let cached: Option<TestData> = cache.get(&key).await?;
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().id, expected_data.id);

    // Second call should hit cache (should be faster)
    let start = std::time::Instant::now();
    let result2 = cache.get_or_set(&key, || async move {
        unreachable!("Should not be called due to cache hit");
    }, Some(Duration::from_secs(300))).await?;
    
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(50)); // Should be much faster
    assert_eq!(result2.id, expected_data.id);

    Ok(())
}

#[tokio::test]
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

        cache.set(&key, &data, Some(Duration::from_secs(300))).await?;
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

        cache.set(&key, &data, Some(Duration::from_secs(60))).await?;
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
async fn test_cdn_manager_configuration() -> Result<(), anyhow::Error> {
    let config = CDNConfig::default();
    let cdn_manager = CDNManager::new(config);

    // Test CDN header generation
    let mut headers = axum::http::HeaderMap::new();
    cdn_manager.add_cdn_headers(&mut headers, aframp_backend::cache::cdn_integration::ResourceType::APIResponse);

    // Verify Cache-Control header
    let cache_control = headers.get("cache-control");
    assert!(cache_control.is_some());
    
    let cache_control_str = cache_control.unwrap().to_str()?;
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
async fn test_cdn_cache_warming() -> Result<(), anyhow::Error> {
    let config = CDNConfig::default();
    let cdn_manager = CDNManager::new(config);

    // Prepare warmup resources
    let mut resources = Vec::new();
    for i in 0..5 {
        resources.push(aframp_backend::cache::cdn_integration::CacheWarmupResource {
            path: format!("/static/test_{}.js", i),
            resource_type: aframp_backend::cache::cdn_integration::ResourceType::StaticAsset,
            priority: aframp_backend::cache::cdn_integration::WarmupPriority::Medium,
            headers: std::collections::HashMap::new(),
        });
    }

    // Warm cache
    cdn_manager.warm_cache(resources).await?;

    // Get metrics
    let metrics = cdn_manager.get_metrics();
    assert!(metrics.enabled);

    Ok(())
}

#[tokio::test]
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
            cache_clone.set(&key, &data, Some(Duration::from_secs(60))).await?;

            // Get value
            let retrieved: Option<TestData> = cache_clone.get(&key).await?;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().value, i);

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
            cache.set(&key, &data, Some(Duration::from_secs(300))).await?;
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
        println!("SET operations: {} ops in {:?} ({:.2} ops/sec)", 
            NUM_OPERATIONS, set_duration, NUM_OPERATIONS as f64 / set_duration.as_secs_f64());
        println!("GET operations: {} ops in {:?} ({:.2} ops/sec)", 
            NUM_OPERATIONS, get_duration, NUM_OPERATIONS as f64 / get_duration.as_secs_f64());
        println!("Batch SET (100 items): {:?}", batch_set_duration);

        // Cleanup
        for i in 0..NUM_OPERATIONS {
            let key = format!("benchmark_set:{}", i);
            cache.delete(&key).await?;
        }

        Ok(())
    }
}
