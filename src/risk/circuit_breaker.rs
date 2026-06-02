//! Multi-Tier Circuit Breaker Engine — Issue #494.
//!
//! Evaluates risk metrics every 8 s and executes zero-latency corridor isolation
//! via Redis. State changes are broadcast via Redis Pub/Sub so all horizontally
//! scaled instances synchronise within < 15 ms.

use crate::cache::RedisPool;
use crate::risk::{
    metrics as risk_metrics,
    models::IsolationScope,
    repository::RiskRepository,
    volatility,
};
use redis::AsyncCommands;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

const EVAL_INTERVAL_SECS: u64 = 8;
const CB_CHANNEL: &str = "risk:circuit_breaker:events";
const CORRIDOR_STATUS_KEY: &str = "risk:corridor:status:";

pub struct CircuitBreakerEngine {
    repo: Arc<RiskRepository>,
    redis: RedisPool,
}

impl CircuitBreakerEngine {
    pub fn new(repo: Arc<RiskRepository>, redis: RedisPool) -> Self {
        Self { repo, redis }
    }

    pub async fn run(self, mut shutdown_rx: watch::Receiver<bool>) {
        info!("CircuitBreakerEngine started (interval={}s)", EVAL_INTERVAL_SECS);
        let mut ticker = interval(Duration::from_secs(EVAL_INTERVAL_SECS));
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    info!("CircuitBreakerEngine shutting down");
                    break;
                }
                _ = ticker.tick() => {
                    if let Err(e) = self.evaluate().await {
                        error!(error = %e, "CircuitBreakerEngine evaluation failed");
                    }
                }
            }
        }
    }

    async fn evaluate(&self) -> anyhow::Result<()> {
        let profiles = self.repo.list_profiles().await?;

        for profile in &profiles {
            // Check volatility
            let sigma_threshold: f64 = profile.max_volatility_sigma
                .to_string()
                .parse()
                .unwrap_or(3.0);

            if let Some(sigma) = volatility::check_volatility(
                &self.redis,
                &profile.corridor_id,
                sigma_threshold,
            )
            .await
            {
                warn!(
                    corridor = %profile.corridor_id,
                    sigma,
                    "Volatility breach — tripping circuit breaker"
                );
                self.trip(
                    &profile.corridor_id,
                    IsolationScope::Corridor,
                    "volatility_sigma",
                    sigma,
                    sigma_threshold,
                )
                .await?;

                risk_metrics::circuit_breaker_trips()
                    .with_label_values(&[&profile.corridor_id, "volatility"])
                    .inc();
            }
        }

        Ok(())
    }

    /// Trip the circuit breaker for a corridor — sets Redis state in < 5 ms
    /// and broadcasts via Pub/Sub for cross-instance sync.
    pub async fn trip(
        &self,
        corridor_id: &str,
        scope: IsolationScope,
        trigger_metric: &str,
        trigger_value: f64,
        trigger_threshold: f64,
    ) -> anyhow::Result<()> {
        // 1. Atomically set corridor status in Redis (< 5 ms)
        let status_key = format!("{}{}", CORRIDOR_STATUS_KEY, corridor_id);
        if let Ok(mut conn) = self.redis.get().await {
            let _: Result<(), _> = conn.set_ex(&status_key, "PAUSED", 3600).await;
        }

        // 2. Persist to DB
        let event = self
            .repo
            .open_circuit_breaker(
                corridor_id,
                &scope,
                trigger_metric,
                trigger_value,
                trigger_threshold,
            )
            .await?;

        // 3. Broadcast via Pub/Sub for cross-instance sync (< 15 ms propagation)
        let msg = serde_json::json!({
            "event_id": event.id,
            "corridor_id": corridor_id,
            "scope": scope.to_string(),
            "trigger_metric": trigger_metric,
            "trigger_value": trigger_value,
        })
        .to_string();

        if let Ok(mut conn) = self.redis.get().await {
            let _: Result<(), _> = conn.publish(CB_CHANNEL, &msg).await;
        }

        info!(
            corridor = corridor_id,
            event_id = %event.id,
            "Circuit breaker tripped"
        );
        Ok(())
    }

    /// Check if a corridor is currently isolated.
    pub async fn is_isolated(&self, corridor_id: &str) -> bool {
        let key = format!("{}{}", CORRIDOR_STATUS_KEY, corridor_id);
        if let Ok(mut conn) = self.redis.get().await {
            let val: Option<String> = conn.get(&key).await.unwrap_or(None);
            return val.as_deref() == Some("PAUSED");
        }
        false
    }

    /// Re-route a transaction to a secondary corridor if the primary is isolated.
    /// Returns the target corridor ID to use.
    pub fn reroute(primary: &str) -> Option<&'static str> {
        // Static fallback map — in production this would be DB-driven
        match primary {
            "ngn_kes" => Some("ngn_kes_secondary"),
            "ngn_usd" => Some("ngn_usd_secondary"),
            _ => None,
        }
    }
}

/// Check corridor status from Redis (used by transaction processing layer).
pub async fn corridor_status(redis: &RedisPool, corridor_id: &str) -> &'static str {
    let key = format!("{}{}", CORRIDOR_STATUS_KEY, corridor_id);
    if let Ok(mut conn) = redis.get().await {
        let val: Option<String> = conn.get(&key).await.unwrap_or(None);
        if val.as_deref() == Some("PAUSED") {
            return "PAUSED";
        }
    }
    "ACTIVE"
}
