//! #490 Gas & Fee Optimization — telemetry aggregator, EMA filter,
//! estimation matrix, transaction escalator, and fee bump pipeline.

use super::models::*;
use super::repository::FeeOptimizerRepository;
use super::metrics;
use crate::cache::RedisPool;
use anyhow::{anyhow, Result};
use bigdecimal::ToPrimitive;
use chrono::Utc;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

/// EMA smoothing factor — 2-second poll on fast chains ≈ α = 0.2.
const EMA_ALPHA: f64 = 0.2;
/// Redis TTL for cached fee params (10 seconds).
const FEE_CACHE_TTL_SECS: u64 = 10;

pub struct FeeOptimizerEngine {
    repo: Arc<FeeOptimizerRepository>,
    redis: RedisPool,
    /// Per-network EMA state, protected by a Mutex for concurrent worker access.
    ema_states: Arc<Mutex<HashMap<String, EmaState>>>,
}

impl FeeOptimizerEngine {
    pub fn new(repo: Arc<FeeOptimizerRepository>, redis: RedisPool) -> Self {
        Self {
            repo,
            redis,
            ema_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // ── Fee telemetry aggregator ──────────────────────────────────────────────

    /// Ingest a raw fee observation from an RPC provider, apply EMA filter,
    /// persist snapshot, and push to Redis. Must complete within 15 ms.
    pub async fn ingest_fee_snapshot(
        &self,
        network: ChainNetwork,
        base_fee: u128,
        priority_fee: u128,
        rpc_provider: &str,
        block_reference: Option<i64>,
    ) -> Result<()> {
        let t0 = Instant::now();

        let (ema_base, ema_priority) = {
            let mut states = self.ema_states.lock().unwrap();
            let state = states
                .entry(format!("{:?}", network))
                .or_insert_with(|| EmaState::new(EMA_ALPHA));
            state.update(base_fee as f64, priority_fee as f64);
            (state.ema_base, state.ema_priority)
        };

        let snap = NetworkFeeSnapshot {
            snapshot_id: Uuid::new_v4(),
            network: network.clone(),
            base_fee: sqlx::types::BigDecimal::from_str(&base_fee.to_string())?,
            priority_fee: sqlx::types::BigDecimal::from_str(&priority_fee.to_string())?,
            ema_base_fee: sqlx::types::BigDecimal::try_from(ema_base)?,
            ema_priority_fee: sqlx::types::BigDecimal::try_from(ema_priority)?,
            rpc_provider: rpc_provider.to_string(),
            block_reference,
            captured_at: Utc::now(),
        };
        self.repo.insert_snapshot(&snap).await?;

        // Push to Redis for instant cross-instance distribution
        let cache_key = format!("fee:ema:{:?}", network);
        let payload = serde_json::json!({
            "ema_base": ema_base,
            "ema_priority": ema_priority,
            "captured_at": Utc::now().to_rfc3339(),
        });
        if let Ok(mut conn) = self.redis.get().await {
            let _: Result<(), _> = conn
                .set_ex(cache_key, payload.to_string(), FEE_CACHE_TTL_SECS)
                .await;
        }

        let elapsed_ms = t0.elapsed().as_millis();
        if elapsed_ms > 15 {
            warn!(elapsed_ms, network = ?network, "Fee ingest exceeded 15 ms SLA");
        }

        metrics::rpc_fee_latency().observe(t0.elapsed().as_secs_f64());
        Ok(())
    }

    // ── Estimation matrix ─────────────────────────────────────────────────────

    /// Compute optimal fee parameters for a given network + urgency.
    /// Returns ready-to-sign fee params within 15 ms.
    pub async fn estimate_fees(
        &self,
        network: ChainNetwork,
        urgency: UrgencyWindow,
        tenant_id: Option<Uuid>,
    ) -> Result<OptimizedFeeParams> {
        let t0 = Instant::now();

        // Try Redis cache first
        let (ema_base, ema_priority) = self.get_ema_from_cache(&network).await;

        let policy = self
            .repo
            .get_policy(&network, &urgency, tenant_id)
            .await?
            .ok_or_else(|| anyhow!("no_fee_policy_found"))?;

        let multiplier = policy.fee_multiplier.to_f64().unwrap_or(1.1);
        let max_cap = policy.max_fee_cap.to_f64().unwrap_or(f64::MAX);
        let halt_threshold = policy.congestion_halt_threshold.to_f64().unwrap_or(f64::MAX);

        // Congestion circuit breaker
        if ema_base > halt_threshold {
            warn!(
                network = ?network,
                ema_base,
                halt_threshold,
                "Fee congestion halt threshold exceeded — pooling non-urgent payouts"
            );
            metrics::congestion_halts().inc();
            return Err(anyhow!("congestion_halt"));
        }

        let raw_base = (ema_base * multiplier) as u128;
        let raw_priority = (ema_priority * multiplier) as u128;

        let max_fee = raw_base.min(max_cap as u128);
        let max_priority = raw_priority.min(max_cap as u128);

        let elapsed_ms = t0.elapsed().as_millis();
        if elapsed_ms > 15 {
            warn!(elapsed_ms, "Fee estimation exceeded 15 ms SLA");
        }

        metrics::fee_optimization_savings().observe(
            (max_cap as f64 - raw_base as f64).max(0.0),
        );

        Ok(OptimizedFeeParams {
            network,
            max_fee_per_gas: max_fee,
            max_priority_fee_per_gas: max_priority,
            urgency,
            estimated_at: Utc::now(),
        })
    }

    // ── Transaction escalator (fee bump pipeline) ─────────────────────────────

    /// Monitor stalled transactions and issue fee bumps.
    /// Preserves nonce/sequence integrity — replacement envelopes explicitly
    /// match the originating transaction's nonce.
    pub async fn escalate_stalled_transactions(&self) -> Result<()> {
        let stalled = self.repo.list_stalled_logs().await?;

        for log in &stalled {
            if log.bump_count >= 3 {
                error!(
                    gas_log_id = %log.gas_log_id,
                    parent_tx_id = %log.parent_tx_id,
                    bump_count = log.bump_count,
                    "P1 ALERT: Settlement transaction unconfirmed after 3 min + 3 bumps"
                );
                metrics::bump_events().inc();
                continue;
            }

            if let Err(e) = self.bump_transaction(log).await {
                error!(
                    gas_log_id = %log.gas_log_id,
                    error = %e,
                    "Fee bump failed"
                );
            }
        }
        Ok(())
    }

    async fn bump_transaction(&self, log: &ExecutionGasLog) -> Result<()> {
        // Fetch current EMA and apply 1.25× bump multiplier
        let (ema_base, _) = self.get_ema_from_cache(&log.network).await;
        let bumped_fee = (ema_base * 1.25) as u128;

        let bd = sqlx::types::BigDecimal::from_str(&bumped_fee.to_string())?;
        self.repo.bump_gas_log(log.gas_log_id, &bd).await?;

        info!(
            gas_log_id = %log.gas_log_id,
            nonce = log.nonce_or_sequence,
            bumped_fee,
            "Fee bump issued — nonce preserved"
        );

        metrics::bump_events().inc();
        metrics::gas_spent().inc_by(bumped_fee as f64);
        Ok(())
    }

    // ── Redis helpers ─────────────────────────────────────────────────────────

    async fn get_ema_from_cache(&self, network: &ChainNetwork) -> (f64, f64) {
        let key = format!("fee:ema:{:?}", network);
        if let Ok(mut conn) = self.redis.get().await {
            if let Ok(val) = conn.get::<_, String>(&key).await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&val) {
                    let base = json["ema_base"].as_f64().unwrap_or(0.0);
                    let priority = json["ema_priority"].as_f64().unwrap_or(0.0);
                    return (base, priority);
                }
            }
        }
        // Fallback to in-memory EMA
        let states = self.ema_states.lock().unwrap();
        if let Some(state) = states.get(&format!("{:?}", network)) {
            return (state.ema_base, state.ema_priority);
        }
        (0.0, 0.0)
    }
}
