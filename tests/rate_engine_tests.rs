//! Integration tests for Issue #530: rate limiting, DRR scheduler, backpressure.

use aframp_backend::rate_engine::{
    models::{RateLimitDecision, TenantSlaProfile},
    scheduler::DrqScheduler,
    token_bucket::TokenBucketLimiter,
};
use uuid::Uuid;

fn profile(baseline: i32, burst: i32, weight: i32) -> TenantSlaProfile {
    TenantSlaProfile {
        tenant_id: Uuid::new_v4(),
        tier: "standard".into(),
        max_concurrent_conns: 100,
        baseline_rps: baseline,
        burst_rps: burst,
        queue_weight: weight,
        burst_window_ms: 5000,
        enabled: true,
    }
}

#[tokio::test]
async fn test_rogue_tenant_does_not_starve_others() {
    let limiter = TokenBucketLimiter::new(None, 10_000.0);

    let rogue    = profile(10, 3, 1);   // tiny burst capacity
    let baseline = profile(100, 50, 10);

    // Exhaust rogue tenant
    for _ in 0..3 {
        let _ = limiter.check(&rogue).await;
    }
    let rogue_verdict = limiter.check(&rogue).await;
    assert!(matches!(rogue_verdict, RateLimitDecision::Throttle { .. }),
        "rogue tenant should be throttled");

    // Baseline tenant should still be allowed
    let base_verdict = limiter.check(&baseline).await;
    assert_eq!(base_verdict, RateLimitDecision::Allow,
        "baseline tenant must not be affected by rogue tenant");
}

#[tokio::test]
async fn test_queue_evacuation_prevents_head_of_line_blocking() {
    let scheduler = DrqScheduler::new(10);

    let t1 = profile(100, 200, 10);
    let t2 = profile(100, 200, 10);
    scheduler.register_tenant(&t1).await;
    scheduler.register_tenant(&t2).await;

    // Evacuate t1's corridor
    scheduler.evacuate_corridor(t1.tenant_id, "downstream offline").await;

    // t1 enqueue should fail (evacuated)
    assert!(!scheduler.enqueue(t1.tenant_id, serde_json::json!({"tx":"x"})).await);
    // t2 enqueue should succeed
    assert!(scheduler.enqueue(t2.tenant_id, serde_json::json!({"tx":"y"})).await);
}
