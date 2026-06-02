//! Breach Response Engine — Issue #464
//!
//! Evaluates active SLA policies every 10 seconds against Redis sliding-window
//! aggregates. On breach:
//!   1. Persists a `sla_breach_events` row (immutable; overrides create audit entries).
//!   2. Fires signed webhook payloads to institutional partner endpoints.
//!   3. Engages circuit-breaker routing by writing corridor status to Redis.
//!   4. Emits Prometheus metrics.

use crate::cache::RedisPool;
use crate::sla::{aggregator, metrics as sla_metrics, repository::SlaRepository};
use redis::AsyncCommands;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

const EVAL_INTERVAL_SECS: u64 = 10;
/// Redis key prefix for corridor circuit-breaker state.
pub const CB_KEY_PREFIX: &str = "sla:cb:corridor:";

pub struct BreachResponseEngine {
    repo: Arc<SlaRepository>,
    redis: RedisPool,
    http: Client,
    webhook_secret: String,
}

impl BreachResponseEngine {
    pub fn new(repo: Arc<SlaRepository>, redis: RedisPool) -> Self {
        Self {
            repo,
            redis,
            http: Client::new(),
            webhook_secret: std::env::var("SLA_WEBHOOK_SECRET").unwrap_or_default(),
        }
    }

    pub async fn run(self, mut shutdown_rx: watch::Receiver<bool>) {
        info!("BreachResponseEngine started (interval={}s)", EVAL_INTERVAL_SECS);
        let mut ticker = interval(Duration::from_secs(EVAL_INTERVAL_SECS));
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    info!("BreachResponseEngine shutting down");
                    break;
                }
                _ = ticker.tick() => {
                    if let Err(e) = self.evaluate().await {
                        error!(error = %e, "BreachResponseEngine evaluation failed");
                    }
                }
            }
        }
    }

    async fn evaluate(&self) -> anyhow::Result<()> {
        let policies = self.repo.list_active_policies().await?;

        for policy in &policies {
            // Pull live P95/P99 from Redis window
            let Some((p95, p99)) = aggregator::percentiles(&self.redis, &policy.corridor_id).await
            else {
                continue;
            };

            let observed = if policy.metric == "p99" { p99 } else { p95 };
            let threshold = policy.threshold_ms as f64;

            sla_metrics::current_latency()
                .with_label_values(&[&policy.corridor_id, &policy.metric])
                .set(observed / 1000.0); // expose as seconds

            if observed <= threshold {
                // Corridor healthy — clear any active circuit-breaker
                self.clear_circuit_breaker(&policy.corridor_id).await;
                continue;
            }

            warn!(
                corridor = %policy.corridor_id,
                observed_ms = observed,
                threshold_ms = threshold,
                "SLA breach detected"
            );

            // 1. Persist breach event
            let breach_id = self
                .repo
                .insert_breach_event(policy.id, &policy.corridor_id, observed, threshold)
                .await?;

            sla_metrics::active_breaches()
                .with_label_values(&[&policy.corridor_id])
                .inc();

            // 2. Engage circuit-breaker (< 15 s from detection)
            self.engage_circuit_breaker(&policy.corridor_id).await;

            // 3. Fire webhook notifications (fire-and-forget)
            let endpoints = self.repo.list_partner_webhook_endpoints(&policy.corridor_id).await.unwrap_or_default();
            for endpoint in endpoints {
                let payload = self.build_webhook_payload(breach_id, &policy.corridor_id, observed, threshold);
                let sig = self.sign_payload(&payload);
                let http = self.http.clone();
                let url = endpoint.clone();
                tokio::spawn(async move {
                    let _ = http
                        .post(&url)
                        .header("X-Aframp-Signature", sig)
                        .header("Content-Type", "application/json")
                        .body(payload)
                        .send()
                        .await;
                });
            }

            info!(breach_id = %breach_id, corridor = %policy.corridor_id, "Breach event recorded, circuit-breaker engaged");
        }

        // Update compliance ratio metric
        let total = policies.len() as f64;
        if total > 0.0 {
            let breached = sla_metrics::active_breaches_count();
            let ratio = ((total - breached) / total) * 100.0;
            sla_metrics::compliance_ratio().set(ratio);
        }

        Ok(())
    }

    async fn engage_circuit_breaker(&self, corridor: &str) {
        let key = format!("{}{}", CB_KEY_PREFIX, corridor);
        if let Ok(mut conn) = self.redis.get().await {
            let _: Result<(), _> = conn.set_ex(&key, "PAUSED", 300).await;
        }
        info!(corridor, "Circuit-breaker engaged (PAUSED)");
    }

    async fn clear_circuit_breaker(&self, corridor: &str) {
        let key = format!("{}{}", CB_KEY_PREFIX, corridor);
        if let Ok(mut conn) = self.redis.get().await {
            let _: Result<(), _> = conn.del(&key).await;
        }
    }

    fn build_webhook_payload(&self, breach_id: Uuid, corridor: &str, observed: f64, threshold: f64) -> String {
        serde_json::json!({
            "event": "sla.breach",
            "breach_id": breach_id,
            "corridor": corridor,
            "observed_ms": observed,
            "threshold_ms": threshold,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })
        .to_string()
    }

    fn sign_payload(&self, payload: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(self.webhook_secret.as_bytes())
            .unwrap_or_else(|_| HmacSha256::new_from_slice(b"default").unwrap());
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

/// Check if a corridor's circuit-breaker is currently engaged.
pub async fn is_circuit_open(redis: &RedisPool, corridor: &str) -> bool {
    let key = format!("{}{}", CB_KEY_PREFIX, corridor);
    if let Ok(mut conn) = redis.get().await {
        let val: Option<String> = conn.get(&key).await.unwrap_or(None);
        return val.as_deref() == Some("PAUSED");
    }
    false
}
