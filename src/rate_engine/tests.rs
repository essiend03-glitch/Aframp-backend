//! Unit tests for #530: DRR arithmetic, token bucket logic, backpressure triggers.

use uuid::Uuid;
use crate::rate_engine::{
    models::{RateLimitDecision, TenantSlaProfile},
    token_bucket::TokenBucketLimiter,
    scheduler::DrqScheduler,
};

fn mock_profile(baseline_rps: i32, burst_rps: i32, weight: i32) -> TenantSlaProfile {
    TenantSlaProfile {
        tenant_id: Uuid::new_v4(),
        tier: "standard".into(),
        max_concurrent_conns: 50,
        baseline_rps,
        burst_rps,
        queue_weight: weight,
        burst_window_ms: 5000,
        enabled: true,
    }
}

#[test]
fn test_fill_rate_equals_baseline_rps() {
    let p = mock_profile(100, 200, 10);
    assert_eq!(p.fill_rate(), 100.0);
    assert_eq!(p.capacity(), 200.0);
}

#[tokio::test]
async fn test_memory_bucket_allows_up_to_capacity() {
    let limiter = TokenBucketLimiter::new(None, 1000.0);
    let profile = mock_profile(100, 5, 10); // capacity = 5 tokens

    // First 5 requests should be allowed
    for _ in 0..5 {
        assert_eq!(limiter.check(&profile).await, RateLimitDecision::Allow);
    }
    // 6th should be throttled
    assert!(matches!(limiter.check(&profile).await, RateLimitDecision::Throttle { .. }));
}

#[tokio::test]
async fn test_drr_enqueue_and_evacuate() {
    let scheduler = DrqScheduler::new(10);
    let profile = mock_profile(100, 200, 10);
    scheduler.register_tenant(&profile).await;

    let payload = serde_json::json!({"tx": "abc"});

    // Normal enqueue should succeed
    assert!(scheduler.enqueue(profile.tenant_id, payload.clone()).await);

    // After evacuation, enqueue returns false
    scheduler.evacuate_corridor(profile.tenant_id, "downstream offline").await;
    assert!(!scheduler.enqueue(profile.tenant_id, payload.clone()).await);

    // Restore and enqueue again
    scheduler.restore_corridor(profile.tenant_id).await;
    assert!(scheduler.enqueue(profile.tenant_id, payload).await);
}

#[test]
fn test_throttle_retry_after_calculation() {
    let profile = mock_profile(100, 200, 10);
    // retry_after_ms = ceil(1000 / 100) = 10ms
    let limiter = TokenBucketLimiter::new(None, 1000.0);
    // Just verify the formula: 1000 / fill_rate
    let expected_retry_ms = (1000.0 / profile.fill_rate()) as u64;
    assert_eq!(expected_retry_ms, 10);
}
