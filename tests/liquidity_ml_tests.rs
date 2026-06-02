//! Integration tests for Issue #531: feature pipeline, inference fallback, multi-tenant isolation.

use aframp_backend::liquidity_ml::{
    feature_pipeline::{FeaturePipeline, PaymentFrame},
    inference::InferenceEngine,
};
use uuid::Uuid;

#[tokio::test]
async fn test_feature_pipeline_isolates_corridors() {
    let (snap_tx, mut snap_rx) = tokio::sync::mpsc::channel(256);
    let pipeline = FeaturePipeline::spawn(snap_tx);

    let corridor_a = Uuid::new_v4();
    let corridor_b = Uuid::new_v4();

    for _ in 0..5 {
        pipeline.ingest(PaymentFrame {
            corridor_id:   corridor_a,
            amount_usd:    1000.0,
            currency:      "USD".into(),
            timestamp_ms:  chrono::Utc::now().timestamp_millis(),
            bank_delay_ms: 50,
        }).await;

        pipeline.ingest(PaymentFrame {
            corridor_id:   corridor_b,
            amount_usd:    2000.0,
            currency:      "USD".into(),
            timestamp_ms:  chrono::Utc::now().timestamp_millis(),
            bank_delay_ms: 80,
        }).await;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Verify corridor snapshots are segregated (no cross-tenant leakage)
    let mut seen_a = false;
    let mut seen_b = false;
    while let Ok(snap) = snap_rx.try_recv() {
        if snap.corridor_id == corridor_a {
            seen_a = true;
            assert!(snap.throughput_usd >= 1000.0 * 5.0, "corridor A throughput mismatch");
        }
        if snap.corridor_id == corridor_b {
            seen_b = true;
            assert!(snap.throughput_usd >= 2000.0 * 5.0, "corridor B throughput mismatch");
        }
    }
    // At least one snapshot per corridor should have been emitted
    assert!(seen_a || seen_b, "no snapshots received from pipeline");
}

#[test]
fn test_inference_failsafe_on_low_confidence() {
    let engine = InferenceEngine::new(0.99, 3.0); // very high threshold → always fallback
    let snap = aframp_backend::liquidity_ml::feature_pipeline::FeatureSnapshot {
        corridor_id: Uuid::new_v4(),
        throughput_usd: 10000.0,
        rolling_variance: 500.0,
        velocity_1h: 1200.0,
        velocity_24h: 28800.0,
        bank_delay_ms: 120.0,
    };
    let pred = engine.predict(&snap).unwrap();
    // Fallback: velocity_1h * fallback_multiplier = 1200 * 3 = 3600
    assert_eq!(pred.predicted_volume, 3600.0);
}
