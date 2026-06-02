//! Embedded inference engine with fail-secure fallback to static threshold rules.
//! Uses a mock runtime here (in production: ort or tract crate with ONNX models).

use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use super::feature_pipeline::FeatureSnapshot;

/// Inference output: predicted volume draw-down in USD over next `window_hours`.
#[derive(Debug, Clone)]
pub struct LiquidityPrediction {
    pub corridor_id:       Uuid,
    pub predicted_volume:  f64,
    pub prediction_window_hours: i32,
    pub confidence_score:  f64, // [0.0, 1.0]
}

/// Mock inference engine. In production, this loads ONNX weights with SHA-256 verification.
pub struct InferenceEngine {
    confidence_threshold: f64,
    fallback_multiplier:  f64,
}

impl InferenceEngine {
    pub fn new(confidence_threshold: f64, fallback_multiplier: f64) -> Self {
        Self { confidence_threshold, fallback_multiplier }
    }

    /// Run forward pass on the feature snapshot.
    /// If confidence < threshold, falls back to conservative static rule.
    pub fn predict(&self, snap: &FeatureSnapshot) -> Result<LiquidityPrediction, String> {
        // MOCK: simple linear extrapolation for demo
        let predicted_volume = snap.velocity_1h * 6.0; // 6h projection
        let confidence = 0.85; // mock confidence

        if confidence < self.confidence_threshold {
            warn!(corridor=%snap.corridor_id, conf=%confidence, "confidence below threshold – falling back");
            return Ok(LiquidityPrediction {
                corridor_id: snap.corridor_id,
                predicted_volume: snap.velocity_1h * self.fallback_multiplier,
                prediction_window_hours: 6,
                confidence_score: confidence,
            });
        }

        Ok(LiquidityPrediction {
            corridor_id:       snap.corridor_id,
            predicted_volume,
            prediction_window_hours: 6,
            confidence_score:  confidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_when_confidence_low() {
        let engine = InferenceEngine::new(0.90, 2.0);
        let snap = FeatureSnapshot {
            corridor_id: Uuid::new_v4(),
            throughput_usd: 1000.0,
            rolling_variance: 10.0,
            velocity_1h: 50.0,
            velocity_24h: 1200.0,
            bank_delay_ms: 100.0,
        };
        let pred = engine.predict(&snap).unwrap();
        // confidence = 0.85 < 0.90 → fallback: velocity_1h * 2.0 = 100.0
        assert_eq!(pred.predicted_volume, 100.0);
        assert_eq!(pred.confidence_score, 0.85);
    }
}
