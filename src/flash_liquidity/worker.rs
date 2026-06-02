//! #488 Flash Liquidity — real-time risk monitoring loop + margin circuit breaker.

use super::engine::{FlashLiquidityEngine, MIN_HEALTH_FACTOR};
use super::models::*;
use super::repository::FlashLiquidityRepository;
use super::metrics;
use crate::cache::RedisPool;
use anyhow::Result;
use bigdecimal::{BigDecimal, ToPrimitive};
use redis::AsyncCommands;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

const MONITOR_INTERVAL_SECS: u64 = 10;
const LOCK_TTL_SECS: u64 = 60;

pub struct RiskMonitorWorker {
    repo: Arc<FlashLiquidityRepository>,
    engine: Arc<FlashLiquidityEngine>,
    redis: RedisPool,
}

impl RiskMonitorWorker {
    pub fn new(
        repo: Arc<FlashLiquidityRepository>,
        engine: Arc<FlashLiquidityEngine>,
        redis: RedisPool,
    ) -> Self {
        Self { repo, engine, redis }
    }

    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        let mut ticker = interval(Duration::from_secs(MONITOR_INTERVAL_SECS));
        info!("Flash liquidity risk monitor started");
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = self.tick().await {
                        error!(error = %e, "Risk monitor tick failed");
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Risk monitor shutting down");
                        break;
                    }
                }
            }
        }
    }

    async fn tick(&self) -> Result<()> {
        // Process repayments
        self.engine.process_repayments().await?;

        // Evaluate collateral health for active draws
        let draws = self.repo.list_pending_repayments().await?;
        for draw in &draws {
            if let Err(e) = self.evaluate_health(draw).await {
                error!(draw_id = %draw.draw_id, error = %e, "Health evaluation failed");
            }
        }
        Ok(())
    }

    async fn evaluate_health(&self, draw: &FlashLiquidityDraw) -> Result<()> {
        // Acquire distributed lock to prevent race conditions across instances
        let lock_key = format!("flash:health:lock:{}", draw.draw_id);
        if !self.acquire_lock(&lock_key).await {
            return Ok(());
        }

        // Fetch live collateral price from Redis (set by oracle feed)
        let collateral_price = self.get_collateral_price(&draw.collateral_asset).await;
        let collateral_value = &draw.collateral_amount * BigDecimal::try_from(collateral_price)?;
        let debt = draw.draw_amount.clone();

        // health_factor = collateral_value / debt (simplified; production uses required_dcr)
        let health_factor = if debt > BigDecimal::from(0) {
            collateral_value.to_f64().unwrap_or(0.0) / debt.to_f64().unwrap_or(1.0)
        } else {
            f64::INFINITY
        };

        let near_liquidation = health_factor < MIN_HEALTH_FACTOR;
        let circuit_breaker_action = if near_liquidation {
            warn!(
                draw_id = %draw.draw_id,
                health_factor,
                "P1 ALERT: Collateral health factor near liquidation — initiating top-up"
            );
            metrics::liquidation_defense_actions().inc();
            Some("collateral_top_up_initiated".to_string())
        } else {
            None
        };

        let log = CollateralHealthLog {
            log_id: Uuid::new_v4(),
            draw_id: draw.draw_id,
            collateral_value_usd: collateral_value,
            debt_amount_usd: debt,
            health_factor: BigDecimal::try_from(health_factor)?,
            near_liquidation,
            circuit_breaker_action,
            evaluated_at: chrono::Utc::now(),
        };
        self.repo.insert_health_log(&log).await?;

        metrics::health_factor().set(health_factor);

        self.release_lock(&lock_key).await;
        Ok(())
    }

    async fn get_collateral_price(&self, asset: &str) -> f64 {
        let key = format!("oracle:price:{}", asset);
        if let Ok(mut conn) = self.redis.get().await {
            if let Ok(val) = conn.get::<_, String>(&key).await {
                return val.parse::<f64>().unwrap_or(1.0);
            }
        }
        1.0
    }

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
