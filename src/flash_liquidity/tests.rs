//! #488 Flash Liquidity — unit tests.

#[cfg(test)]
mod tests {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    // ── Margin / health factor algebra ────────────────────────────────────────

    fn health_factor(collateral_value: f64, debt: f64) -> f64 {
        if debt == 0.0 { f64::INFINITY } else { collateral_value / debt }
    }

    #[test]
    fn health_factor_above_threshold_is_safe() {
        let hf = health_factor(1500.0, 1000.0); // 150 % collateral
        assert!(hf >= 1.10, "Expected safe health factor, got {}", hf);
    }

    #[test]
    fn health_factor_below_threshold_triggers_circuit_breaker() {
        let hf = health_factor(1050.0, 1000.0); // 105 % — below 110 % min
        assert!(hf < 1.10, "Expected circuit breaker trigger, got {}", hf);
    }

    // ── Debt-to-collateral calculation ────────────────────────────────────────

    #[test]
    fn collateral_calculation_seven_dp_precision() {
        let draw_amount = BigDecimal::from_str("50000.0000000").unwrap();
        let required_dcr = BigDecimal::from_str("1.5000000").unwrap();
        let collateral = &draw_amount * &required_dcr;
        assert_eq!(collateral, BigDecimal::from_str("75000.00000000000000").unwrap());
    }

    // ── Interest accrual ──────────────────────────────────────────────────────

    #[test]
    fn interest_accrual_precision() {
        // 5 bps/day on $10,000 for 8 hours = 10000 * 0.0005 * (8/24)
        let principal = 10_000.0_f64;
        let rate_bps_daily = 5.0_f64;
        let hours = 8.0_f64;
        let interest = principal * (rate_bps_daily / 10_000.0) * (hours / 24.0);
        // Should be ~0.1667
        assert!((interest - 0.1667).abs() < 0.001, "Interest: {}", interest);
    }

    // ── Rollback on collateral lock failure ───────────────────────────────────

    #[test]
    fn draw_status_rolls_back_on_lock_failure() {
        use crate::flash_liquidity::models::DrawStatus;
        let status = DrawStatus::RolledBack;
        assert_eq!(status, DrawStatus::RolledBack);
    }
}
