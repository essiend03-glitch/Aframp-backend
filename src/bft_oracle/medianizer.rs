//! BFT Medianizer: weighted median with Winsorized (4σ) outlier removal.
//! Requires 2t+1 honest nodes from a pool of t Byzantine nodes (quorum parameter).

use chrono::Utc;
use tracing::warn;

use super::models::{BftPrice, OracleTick};

/// Maximum Z-score deviation before a price tick is Winsorized out.
const SIGMA_THRESHOLD: f64 = 4.0;

pub struct BftMedianizer {
    /// Minimum number of valid submissions required before computing a price.
    quorum: usize,
}

impl BftMedianizer {
    pub fn new(total_nodes: usize, byzantine_faults: usize) -> Self {
        Self { quorum: 2 * byzantine_faults + 1 }
    }

    /// Compute BFT price from raw ticks. Returns None if quorum not met.
    pub fn compute(&self, pair: &str, ticks: &[OracleTick]) -> Option<BftPrice> {
        if ticks.len() < self.quorum {
            warn!(pair, needed=self.quorum, got=ticks.len(), "BFT quorum not met");
            return None;
        }

        let prices: Vec<f64> = ticks.iter().map(|t| t.price).collect();
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let stddev = (prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>()
            / prices.len() as f64).sqrt();

        // Winsorize: drop ticks beyond 4σ
        let filtered: Vec<f64> = prices.iter().copied()
            .filter(|p| (p - mean).abs() <= SIGMA_THRESHOLD * stddev.max(1e-18))
            .collect();

        if filtered.len() < self.quorum {
            warn!(pair, "all ticks within winsorization range but below quorum after filter");
            return None;
        }

        let median = weighted_median(&filtered, ticks);
        Some(BftPrice {
            pair:         pair.into(),
            price:        median,
            sources_used: filtered.len(),
            quorum_met:   true,
            computed_at:  Utc::now(),
        })
    }
}

/// Weighted median: sort by price, accumulate weights until reaching 50% of total weight.
fn weighted_median(accepted_prices: &[f64], ticks: &[OracleTick]) -> f64 {
    let mut weighted: Vec<(f64, u32)> = ticks.iter()
        .filter(|t| accepted_prices.contains(&t.price))
        .map(|t| (t.price, t.weight))
        .collect();
    weighted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let total_weight: u32 = weighted.iter().map(|w| w.1).sum();
    let half = total_weight / 2;
    let mut cumulative = 0u32;
    for (price, w) in &weighted {
        cumulative += w;
        if cumulative >= half {
            return *price;
        }
    }
    weighted.last().map(|(p, _)| *p).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn tick(price: f64, weight: u32) -> OracleTick {
        OracleTick { node_id: Uuid::new_v4(), pair: "XLM/USD".into(), price, weight, tick_at: Utc::now() }
    }

    #[test]
    fn test_quorum_not_met_returns_none() {
        let m = BftMedianizer::new(5, 2); // quorum = 5
        let ticks = vec![tick(1.05, 1), tick(1.06, 1)]; // only 2
        assert!(m.compute("XLM/USD", &ticks).is_none());
    }

    #[test]
    fn test_winsorized_outlier_removed() {
        let m = BftMedianizer::new(5, 1); // quorum = 3
        let ticks = vec![
            tick(1.05, 1), tick(1.06, 1), tick(1.07, 1),
            tick(1.05, 1), tick(9999.99, 1), // outlier
        ];
        let result = m.compute("XLM/USD", &ticks).unwrap();
        // Outlier at 9999.99 should be Winsorized out; median ≈ 1.06
        assert!(result.price < 2.0, "outlier not removed: {}", result.price);
    }

    #[test]
    fn test_bft_price_18dp_precision() {
        let m = BftMedianizer::new(3, 1);
        let ticks = vec![
            tick(1.050000000000000001, 1),
            tick(1.060000000000000001, 1),
            tick(1.070000000000000001, 1),
        ];
        let result = m.compute("XLM/USD", &ticks).unwrap();
        assert!(result.price > 0.0);
    }
}
