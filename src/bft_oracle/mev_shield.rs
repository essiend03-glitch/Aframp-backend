//! MEV protection: flash-loan interceptor and transaction delay randomizer.

use rand::Rng;
use tracing::{info, warn};

/// Result of MEV evaluation.
#[derive(Debug, PartialEq)]
pub enum MevDecision {
    /// Safe to proceed immediately.
    Allow,
    /// Pause execution; spot price spike detected.
    Pause { deviation_pct: f64 },
    /// Apply a random delay to neutralize time-delay arbitrage.
    DelayMs(u64),
}

pub struct MevShield {
    /// % spike threshold that triggers a flash-loan pause.
    flash_loan_threshold_pct: f64,
    /// Max random delay (ms) to apply for HFT neutralization.
    max_delay_ms: u64,
}

impl MevShield {
    pub fn new(flash_loan_threshold_pct: f64, max_delay_ms: u64) -> Self {
        Self { flash_loan_threshold_pct, max_delay_ms }
    }

    /// Check whether a spot price deviation constitutes a flash-loan attack.
    pub fn evaluate_flash_loan(&self, spot_price: f64, baseline_price: f64) -> MevDecision {
        if baseline_price == 0.0 { return MevDecision::Allow; }
        let deviation_pct = ((spot_price - baseline_price) / baseline_price).abs() * 100.0;
        if deviation_pct > self.flash_loan_threshold_pct {
            warn!(deviation_pct, "flash-loan spike detected – pausing settlement");
            MevDecision::Pause { deviation_pct }
        } else {
            MevDecision::Allow
        }
    }

    /// Return a random execution delay to neutralize HFT timing attacks.
    pub fn random_delay(&self) -> MevDecision {
        let delay_ms = rand::thread_rng().gen_range(1..=self.max_delay_ms);
        MevDecision::DelayMs(delay_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_loan_triggers_above_threshold() {
        let shield = MevShield::new(1.5, 100);
        assert_eq!(
            shield.evaluate_flash_loan(1.02, 1.0),
            MevDecision::Pause { deviation_pct: 2.0 }
        );
    }

    #[test]
    fn test_flash_loan_allows_below_threshold() {
        let shield = MevShield::new(1.5, 100);
        assert_eq!(shield.evaluate_flash_loan(1.01, 1.0), MevDecision::Allow);
    }

    #[test]
    fn test_random_delay_within_bounds() {
        let shield = MevShield::new(1.5, 50);
        for _ in 0..100 {
            if let MevDecision::DelayMs(d) = shield.random_delay() {
                assert!(d >= 1 && d <= 50);
            }
        }
    }
}
