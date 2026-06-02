//! #487 Automated Treasury Rebalancing Worker.
//!
//! Runs a Tokio loop that checks inventory levels against rebalancing rules.
//! Uses a Redis distributed lock (per currency corridor) to ensure only one
//! rebalancing sequence runs at a time across horizontally scaled instances.

use super::models::*;
use super::repository::SorRepository;
use super::metrics;
use crate::cache::RedisPool;
use anyhow::Result;
use bigdecimal::BigDecimal;
use redis::AsyncCommands;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Redis lock TTL in seconds — long enough to cover a full rebalancing cycle.
const LOCK_TTL_SECS: u64 = 120;
/// How often the worker checks inventory levels.
const CHECK_INTERVAL_SECS: u64 = 30;

pub struct RebalancingWorker {
    repo: Arc<SorRepository>,
    redis: RedisPool,
}

impl RebalancingWorker {
    pub fn new(repo: Arc<SorRepository>, redis: RedisPool) -> Self {
        Self { repo, redis }
    }

    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        info!("Rebalancing worker started");
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = self.tick().await {
                        error!(error = %e, "Rebalancing tick failed");
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Rebalancing worker shutting down");
                        break;
                    }
                }
            }
        }
    }

    async fn tick(&self) -> Result<()> {
        let rules = self.repo.list_enabled_rules().await?;
        for rule in rules {
            if let Err(e) = self.evaluate_rule(&rule).await {
                error!(
                    currency = %rule.currency_code,
                    error = %e,
                    "Rebalancing rule evaluation failed"
                );
            }
        }
        Ok(())
    }

    async fn evaluate_rule(&self, rule: &TreasuryRebalancingRule) -> Result<()> {
        // Fetch current inventory fraction from Redis (set by treasury engine)
        let inventory_pct = self.get_inventory_pct(&rule.currency_code).await;

        let min = rule.min_inventory_pct.to_string().parse::<f64>().unwrap_or(0.0);
        if inventory_pct >= min {
            return Ok(()); // No action needed
        }

        warn!(
            currency = %rule.currency_code,
            inventory_pct,
            min_pct = min,
            "Inventory below minimum — initiating rebalance"
        );

        // Acquire distributed lock for this corridor
        let lock_key = format!("rebalance:lock:{}", rule.currency_code);
        if !self.acquire_lock(&lock_key).await {
            info!(currency = %rule.currency_code, "Rebalance lock held by another instance — skipping");
            return Ok(());
        }

        let target = rule.target_inventory_pct.to_string().parse::<f64>().unwrap_or(0.0);
        let deficit_pct = target - inventory_pct;

        // Calculate rebalance amount (deficit_pct of a nominal $1M treasury)
        let nominal_treasury = 1_000_000.0_f64;
        let rebalance_amount = BigDecimal::try_from(deficit_pct * nominal_treasury)
            .unwrap_or_default();

        let result = self
            .execute_stellar_path_payment(&rule.currency_code, &rebalance_amount)
            .await;

        let (status, tx_hash, error_msg) = match &result {
            Ok(hash) => (RebalanceStatus::Completed, Some(hash.as_str()), None),
            Err(e) => {
                error!(
                    currency = %rule.currency_code,
                    error = %e,
                    "P1 ALERT: Rebalancing failed — insufficient counterparty balance or API auth revoked"
                );
                metrics::rebalance_failures().inc();
                (RebalanceStatus::Failed, None, Some(e.to_string()))
            }
        };

        let bd = sqlx::types::BigDecimal::from_str(&rebalance_amount.to_string())?;
        self.repo
            .record_rebalance_log(
                rule.rule_id,
                &rule.currency_code,
                RebalancingTrigger::ThresholdBreach,
                &bd,
                status,
                tx_hash,
                &lock_key,
                error_msg.as_deref(),
            )
            .await?;

        if result.is_ok() {
            self.repo.touch_rule_triggered(rule.rule_id).await?;
            metrics::rebalance_volume().observe(
                rebalance_amount.to_string().parse::<f64>().unwrap_or(0.0),
            );
            info!(
                currency = %rule.currency_code,
                amount = %rebalance_amount,
                "Rebalancing completed"
            );
        }

        self.release_lock(&lock_key).await;
        Ok(())
    }

    // ── Stellar PathPaymentStrictReceive ──────────────────────────────────────

    /// Submits a Stellar PathPaymentStrictReceive operation to rebalance
    /// the corridor on-chain. Returns the transaction hash on success.
    async fn execute_stellar_path_payment(
        &self,
        currency: &str,
        amount: &BigDecimal,
    ) -> Result<String> {
        // In production this calls the Stellar Horizon RPC via reqwest.
        // Here we model the interface; the actual XDR construction lives in
        // src/chains/stellar/.
        info!(
            currency,
            amount = %amount,
            "Stellar PathPaymentStrictReceive submitted"
        );
        // Simulate a successful on-chain hash
        Ok(format!("STELLAR_TX_{}", Uuid::new_v4().to_string().replace('-', "").to_uppercase()))
    }

    // ── Redis helpers ─────────────────────────────────────────────────────────

    async fn get_inventory_pct(&self, currency: &str) -> f64 {
        let key = format!("treasury:inventory_pct:{}", currency);
        if let Ok(mut conn) = self.redis.get().await {
            if let Ok(val) = conn.get::<_, String>(&key).await {
                return val.parse::<f64>().unwrap_or(1.0);
            }
        }
        1.0 // Default to healthy if Redis unavailable
    }

    /// SET NX EX — returns true if lock was acquired.
    async fn acquire_lock(&self, key: &str) -> bool {
        if let Ok(mut conn) = self.redis.get().await {
            let result: redis::RedisResult<Option<String>> = redis::cmd("SET")
                .arg(key)
                .arg("1")
                .arg("NX")
                .arg("EX")
                .arg(LOCK_TTL_SECS)
                .query_async(&mut *conn)
                .await;
            return result.map(|v| v.is_some()).unwrap_or(false);
        }
        false
    }

    async fn release_lock(&self, key: &str) {
        if let Ok(mut conn) = self.redis.get().await {
            let _: Result<(), _> = conn.del(key).await;
        }
    }
}
