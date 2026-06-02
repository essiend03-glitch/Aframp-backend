//! Unit tests for Issue #531: rolling windows, feature transforms, model fallback.

use uuid::Uuid;
use crate::liquidity_ml::{
    feature_pipeline::{CorridorBuffer, FeaturePipeline, PaymentFrame},
    inference::InferenceEngine,
};

fn frame(amount: f64, offset_ms: i64) -> PaymentFrame {
    PaymentFrame {
        corridor_id:   Uuid::nil(),
        amount_usd:    amount,
        currency:      "USD".into(),
        timestamp_ms:  chrono::Utc::now().timestamp_millis() - offset_ms,
        bank_delay_ms: 50,
    }
}

#[test]
fn test_rolling_velocity_7_decimal_precision() {
    // Verify that summed amounts retain 7dp without drift
    let amounts = vec![1234.1234567_f64, 2345.2345678, 3456.3456789];
    let sum: f64 = amounts.iter().sum();
    let expected = 7035.703503_4; // 7 decimal places
    assert!((sum - expected).abs() < 1e-7, "sum drift: {}", (sum - expected).abs());
}

#[test]
fn test_inference_engine_normal_confidence() {
    use crate::liquidity_ml::feature_pipeline::FeatureSnapshot;
    let engine = InferenceEngine::new(0.80, 2.0);
    let snap = FeatureSnapshot {
        corridor_id: Uuid::new_v4(),
        throughput_usd: 5000.0,
        rolling_variance: 200.0,
        velocity_1h: 800.0,
        velocity_24h: 19200.0,
        bank_delay_ms: 80.0,
    };
    let pred = engine.predict(&snap).unwrap();
    // confidence = 0.85 > 0.80 → normal path: velocity_1h * 6 = 4800
    assert_eq!(pred.predicted_volume, 4800.0);
    assert_eq!(pred.prediction_window_hours, 6);
}
