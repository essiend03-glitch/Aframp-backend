//! Volatility Scanner — Issue #494.
//!
//! Tracks rolling rate variances over a 5-minute window and flags deviations
//! exceeding 3σ. All calculations maintain 7 decimal places of precision.

use crate::cache::RedisPool;
use redis::AsyncCommands;
use tracing::warn;

const RATE_WINDOW_KEY: &str = "risk:rates:";
const WINDOW_SECS: u64 = 300; // 5 minutes

/// Push a new rate observation for a currency pair.
pub async fn record_rate(redis: &RedisPool, pair: &str, rate: f64) {
    let key = format!("{}{}", RATE_WINDOW_KEY, pair);
    let now = chrono::Utc::now().timestamp_millis();
    let member = format!("{}:{:.7}", now, rate);

    if let Ok(mut conn) = redis.get().await {
        let _: Result<(), _> = conn.zadd(&key, &member, now as f64).await;
        let cutoff = (now - (WINDOW_SECS as i64 * 1000)) as f64;
        let _: Result<(), _> = conn.zrembyscore(&key, 0.0_f64, cutoff).await;
        let _: Result<(), _> = conn.expire(&key, (WINDOW_SECS * 2) as i64).await;
    }
}

/// Returns `Some(sigma)` if the latest rate deviates more than `threshold_sigma`
/// standard deviations from the window mean. Returns `None` if insufficient data.
pub async fn check_volatility(
    redis: &RedisPool,
    pair: &str,
    threshold_sigma: f64,
) -> Option<f64> {
    let key = format!("{}{}", RATE_WINDOW_KEY, pair);
    let mut conn = redis.get().await.ok()?;

    let members: Vec<String> = conn.zrange(&key, 0, -1).await.unwrap_or_default();
    if members.len() < 10 {
        return None; // need at least 10 samples
    }

    let rates: Vec<f64> = members
        .iter()
        .filter_map(|m| m.split(':').nth(1).and_then(|v| v.parse().ok()))
        .collect();

    let sigma = std_dev(&rates);
    if sigma == 0.0 {
        return None;
    }

    let mean = mean(&rates);
    let latest = *rates.last()?;
    let deviation = ((latest - mean) / sigma).abs();

    if deviation > threshold_sigma {
        warn!(
            pair,
            deviation,
            threshold_sigma,
            latest,
            mean,
            sigma,
            "Volatility threshold exceeded"
        );
        Some(deviation)
    } else {
        None
    }
}

// ── Statistical helpers ───────────────────────────────────────────────────────

pub fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().sum::<f64>() / data.len() as f64
}

pub fn std_dev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let m = mean(data);
    let variance = data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (data.len() - 1) as f64;
    variance.sqrt()
}

/// Volatility scanner worker — polls Redis windows every 30 s.
pub struct VolatilityScanner {
    redis: RedisPool,
    pairs: Vec<String>,
    threshold_sigma: f64,
    on_breach: Box<dyn Fn(String, f64) + Send + Sync>,
}

impl VolatilityScanner {
    pub fn new(
        redis: RedisPool,
        pairs: Vec<String>,
        threshold_sigma: f64,
        on_breach: impl Fn(String, f64) + Send + Sync + 'static,
    ) -> Self {
        Self {
            redis,
            pairs,
            threshold_sigma,
            on_breach: Box::new(on_breach),
        }
    }

    pub async fn run(self, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) {
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(30));
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => break,
                _ = ticker.tick() => {
                    for pair in &self.pairs {
                        if let Some(sigma) = check_volatility(&self.redis, pair, self.threshold_sigma).await {
                            (self.on_breach)(pair.clone(), sigma);
                        }
                    }
                }
            }
        }
    }
}
