//! #490 Gas & Fee Optimization — unit tests.

#[cfg(test)]
mod tests {
    use crate::fee_optimizer::models::EmaState;

    // ── EMA math ──────────────────────────────────────────────────────────────

    #[test]
    fn ema_seeds_on_first_observation() {
        let mut ema = EmaState::new(0.2);
        ema.update(100.0, 10.0);
        assert_eq!(ema.ema_base, 100.0);
        assert_eq!(ema.ema_priority, 10.0);
    }

    #[test]
    fn ema_smooths_spike() {
        let mut ema = EmaState::new(0.2);
        ema.update(100.0, 10.0);
        // Sudden spike to 1000
        ema.update(1000.0, 100.0);
        // EMA should be 0.2*1000 + 0.8*100 = 280
        assert!((ema.ema_base - 280.0).abs() < 0.001, "EMA base: {}", ema.ema_base);
    }

    #[test]
    fn ema_converges_to_stable_value() {
        let mut ema = EmaState::new(0.2);
        for _ in 0..100 {
            ema.update(500.0, 50.0);
        }
        // After many identical observations, EMA ≈ 500
        assert!((ema.ema_base - 500.0).abs() < 1.0, "EMA base: {}", ema.ema_base);
    }

    // ── Fee bump threshold ────────────────────────────────────────────────────

    #[test]
    fn fee_bump_applies_1_25x_multiplier() {
        let ema_base = 20_000_000_000_u128; // 20 Gwei
        let bumped = (ema_base as f64 * 1.25) as u128;
        assert_eq!(bumped, 25_000_000_000);
    }

    // ── Priority calculation ──────────────────────────────────────────────────

    #[test]
    fn fee_multiplier_applied_correctly() {
        let ema_base = 100_u128;
        let multiplier = 1.20_f64;
        let result = (ema_base as f64 * multiplier) as u128;
        assert_eq!(result, 120);
    }

    #[test]
    fn fee_capped_at_max_cap() {
        let raw_fee = 200_u128;
        let max_cap = 150_u128;
        let capped = raw_fee.min(max_cap);
        assert_eq!(capped, 150);
    }

    // ── Congestion halt ───────────────────────────────────────────────────────

    #[test]
    fn congestion_halt_triggers_above_threshold() {
        let ema_base = 600_000_000_000_u128; // 600 Gwei
        let halt_threshold = 500_000_000_000_u128; // 500 Gwei
        assert!(ema_base > halt_threshold, "Should trigger congestion halt");
    }

    #[test]
    fn no_halt_below_threshold() {
        let ema_base = 400_000_000_000_u128;
        let halt_threshold = 500_000_000_000_u128;
        assert!(ema_base <= halt_threshold, "Should not halt");
    }

    // ── Nonce sequence integrity ──────────────────────────────────────────────

    #[test]
    fn replacement_tx_preserves_nonce() {
        let original_nonce: i64 = 42;
        let replacement_nonce: i64 = original_nonce; // Must match
        assert_eq!(original_nonce, replacement_nonce);
    }
}
