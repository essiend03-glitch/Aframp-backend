use crate::agent_cfo::types::AgentKeyState;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{info, warn};
use uuid::Uuid;

/// Burn-rate safety policy.
pub struct WatchdogConfig {
    /// How often the watchdog sweeps all active agents.
    pub poll_interval: Duration,
    /// If an agent spends more than this many cNGN within `window`, freeze it.
    pub max_spend_in_window: Decimal,
    /// Rolling window for burn-rate calculation (seconds).
    pub window_secs: i64,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(60),
            max_spend_in_window: Decimal::from(50), // 50 cNGN / minute
            window_secs: 60,
        }
    }
}

impl WatchdogConfig {
    pub fn from_env() -> Self {
        let max = std::env::var("AGENT_CFO_WATCHDOG_MAX_SPEND")
            .ok()
            .and_then(|v| Decimal::from_str(&v).ok())
            .unwrap_or(Decimal::from(50));
        let window = std::env::var("AGENT_CFO_WATCHDOG_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60i64);
        let poll = std::env::var("AGENT_CFO_WATCHDOG_POLL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);
        Self {
            poll_interval: Duration::from_secs(poll),
            max_spend_in_window: max,
            window_secs: window,
        }
    }
}

pub struct BurnRateWatchdog {
    db: PgPool,
    config: WatchdogConfig,
}

impl BurnRateWatchdog {
    pub fn new(db: PgPool, config: WatchdogConfig) -> Self {
        Self { db, config }
    }

    /// Check one agent; freeze its keys if burn rate is violated.
    /// Returns `true` if the agent was frozen.
    pub async fn check_agent(&self, agent_id: Uuid) -> bool {
        let result = sqlx::query!(
            r#"
            SELECT COALESCE(SUM(cost_cngn::numeric), 0)::text AS total
            FROM agent_inference_events
            WHERE agent_id = $1
              AND recorded_at >= NOW() - ($2 || ' seconds')::interval
            "#,
            agent_id,
            self.config.window_secs.to_string(),
        )
        .fetch_one(&self.db)
        .await;

        let spend = match result {
            Ok(row) => Decimal::from_str(row.total.as_deref().unwrap_or("0"))
                .unwrap_or(Decimal::ZERO),
            Err(e) => {
                warn!(agent_id = %agent_id, error = %e, "watchdog: spend query failed");
                return false;
            }
        };

        if spend > self.config.max_spend_in_window {
            warn!(
                agent_id = %agent_id,
                spend = %spend,
                limit = %self.config.max_spend_in_window,
                "🚨 Runaway burn-rate detected — freezing agent signing keys"
            );
            let _ = sqlx::query!(
                "UPDATE agent_budget_policies SET key_state = 'frozen', updated_at = NOW() \
                 WHERE agent_id = $1 AND key_state = 'active'",
                agent_id,
            )
            .execute(&self.db)
            .await;
            return true;
        }
        false
    }

    /// Background sweep — runs every `poll_interval`.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let mut ticker = tokio::time::interval(self.config.poll_interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let agents = sqlx::query_scalar!(
                        "SELECT agent_id FROM agent_budget_policies WHERE key_state = 'active'"
                    )
                    .fetch_all(&self.db)
                    .await
                    .unwrap_or_default();

                    for agent_id in agents {
                        self.check_agent(agent_id).await;
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Agent CFO watchdog shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Unfreeze an agent (called by admin after manual review).
    pub async fn unfreeze(db: &PgPool, agent_id: Uuid) -> Result<(), String> {
        sqlx::query!(
            "UPDATE agent_budget_policies SET key_state = 'active', updated_at = NOW() \
             WHERE agent_id = $1",
            agent_id,
        )
        .execute(db)
        .await
        .map(|_| ())
        .map_err(|e| format!("unfreeze failed: {e}"))
    }

    pub fn key_state_from_db(state: &str) -> AgentKeyState {
        match state {
            "frozen" => AgentKeyState::Frozen,
            _ => AgentKeyState::Active,
        }
    }
}
