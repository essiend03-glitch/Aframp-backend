use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};
use rand::{thread_rng, Rng};

#[derive(Clone, Debug)]
pub enum OrderSide {
    Buy,
    Sell,
}

pub fn midpoint_vwap(
    bids: &[(BigDecimal, BigDecimal)],
    asks: &[(BigDecimal, BigDecimal)],
) -> Option<BigDecimal> {
    fn vwap(book: &[(BigDecimal, BigDecimal)]) -> Option<BigDecimal> {
        let mut total = BigDecimal::from(0);
        let mut size = BigDecimal::from(0);
        for (price, qty) in book {
            total += price * qty;
            size += qty;
        }
        if size == BigDecimal::from(0) {
            return None;
        }
        Some((total / size).with_scale(7))
    }
    let bid = vwap(bids)?;
    let ask = vwap(asks)?;
    Some(((bid + ask) / BigDecimal::from(2)).with_scale(7))
}

pub fn mask_order_sizes(amount: &BigDecimal, parts: usize) -> Vec<BigDecimal> {
    let parts = parts.max(2);
    let mut rng = thread_rng();
    let ratios: Vec<f64> = (0..parts).map(|_| rng.gen_range(0.8..1.2)).collect();
    let sum: f64 = ratios.iter().sum();
    let mut chunks: Vec<BigDecimal> = ratios
        .into_iter()
        .map(|r| {
            amount
                * BigDecimal::from_f64(r / sum).unwrap_or_else(|| BigDecimal::from(1))
        })
        .collect();
    let total = chunks
        .iter()
        .cloned()
        .reduce(|a, b| a + b)
        .unwrap_or_else(|| BigDecimal::from(0));
    if let Some(last) = chunks.last_mut() {
        *last += amount - total;
    }
    chunks.into_iter().map(|c| c.with_scale(7)).collect()
}

pub fn randomized_delay_ms() -> u64 {
    thread_rng().gen_range(50..=500)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::FromPrimitive;

    fn p(v: f64) -> BigDecimal {
        BigDecimal::from_f64(v).unwrap().with_scale(7)
    }

    #[test]
    fn midpoint_vwap_computes_average() {
        let bid = vec![(p(100.0), p(5.0)), (p(99.5), p(5.0))];
        let ask = vec![(p(101.0), p(5.0)), (p(101.5), p(5.0))];
        let result = midpoint_vwap(&bid, &ask).unwrap();
        assert_eq!(result, p(100.25));
    }

    #[test]
    fn mask_order_sizes_preserves_total() {
        let amount = p(1000.0);
        let chunks = mask_order_sizes(&amount, 4);
        let total = chunks.into_iter().reduce(|a, b| a + b).unwrap();
        assert_eq!(total.with_scale(7), amount);
    }

    #[test]
    fn randomized_delay_ms_is_in_range() {
        let d = randomized_delay_ms();
        assert!(d >= 50 && d <= 500);
    }
}
