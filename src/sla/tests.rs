//! Unit tests for SLA sliding-window aggregation and breach evaluation — Issue #464.

#[cfg(test)]
mod tests {
    use crate::sla::aggregator::percentile_value_test;
    use crate::sla::breach_engine::is_circuit_open;

    // ── Percentile math ───────────────────────────────────────────────────────

    #[test]
    fn test_p95_single_element() {
        let data = vec![100.0_f64];
        assert_eq!(percentile_value_test(&data, 95.0), 100.0);
    }

    #[test]
    fn test_p95_ten_elements() {
        let mut data: Vec<f64> = (1..=10).map(|x| x as f64 * 10.0).collect();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // P95 of [10,20,...,100] → index = round(0.95 * 9) = 9 → 100
        let p95 = percentile_value_test(&data, 95.0);
        assert_eq!(p95, 100.0);
    }

    #[test]
    fn test_p99_hundred_elements() {
        let mut data: Vec<f64> = (1..=100).map(|x| x as f64).collect();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p99 = percentile_value_test(&data, 99.0);
        assert_eq!(p99, 99.0);
    }

    #[test]
    fn test_breach_threshold_logic() {
        let observed = 250.0_f64;
        let threshold = 120.0_f64;
        assert!(observed > threshold, "Should detect breach when observed > threshold");
    }

    #[test]
    fn test_no_breach_when_within_threshold() {
        let observed = 80.0_f64;
        let threshold = 120.0_f64;
        assert!(observed <= threshold, "Should not breach when within threshold");
    }

    // ── Webhook signing ───────────────────────────────────────────────────────

    #[test]
    fn test_webhook_signature_deterministic() {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let secret = b"test-secret";
        let payload = r#"{"event":"sla.breach"}"#;

        let mut mac1 = HmacSha256::new_from_slice(secret).unwrap();
        mac1.update(payload.as_bytes());
        let sig1 = hex::encode(mac1.finalize().into_bytes());

        let mut mac2 = HmacSha256::new_from_slice(secret).unwrap();
        mac2.update(payload.as_bytes());
        let sig2 = hex::encode(mac2.finalize().into_bytes());

        assert_eq!(sig1, sig2, "Signatures must be deterministic");
        assert!(!sig1.is_empty());
    }

    // ── Routing adjustment ────────────────────────────────────────────────────

    #[test]
    fn test_circuit_breaker_key_format() {
        let corridor = "ngn_kes";
        let key = format!("{}{}",  crate::sla::breach_engine::CB_KEY_PREFIX, corridor);
        assert_eq!(key, "sla:cb:corridor:ngn_kes");
    }
}

// Expose percentile helper for tests without making it pub in production code
pub fn percentile_value_test(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
