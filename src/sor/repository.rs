//! #487 Smart Order Routing — database repository.

use super::models::*;
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

pub struct SorRepository {
    pool: PgPool,
}

impl SorRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Venues ────────────────────────────────────────────────────────────────

    pub async fn list_active_venues(&self) -> Result<Vec<LiquidityVenue>> {
        let rows = sqlx::query_as!(
            LiquidityVenue,
            r#"SELECT venue_id, name,
                      venue_type AS "venue_type: VenueType",
                      status AS "status: VenueStatus",
                      api_endpoint,
                      supported_currencies,
                      daily_volume_limit, used_volume_today,
                      execution_fee_bps, spread_bps,
                      last_heartbeat_at, created_at, updated_at
               FROM liquidity_venues
               WHERE status = 'active'"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_venue_heartbeat(&self, venue_id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE liquidity_venues SET last_heartbeat_at = NOW(), updated_at = NOW()
             WHERE venue_id = $1",
            venue_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Executions ────────────────────────────────────────────────────────────

    pub async fn insert_execution(&self, exec: &SmartOrderExecution) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO smart_order_executions
               (execution_id, parent_transaction_id, correlation_tag,
                source_currency, target_currency, total_amount, status,
                routing_plan, path_calc_ms)
               VALUES ($1,$2,$3,$4,$5,$6,$7::sor_status,$8,$9)"#,
            exec.execution_id,
            exec.parent_transaction_id,
            exec.correlation_tag,
            exec.source_currency,
            exec.target_currency,
            exec.total_amount,
            exec.status.clone() as SorStatus,
            exec.routing_plan,
            exec.path_calc_ms,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_execution_status(
        &self,
        execution_id: Uuid,
        status: SorStatus,
        slippage_bps: Option<f64>,
    ) -> Result<()> {
        sqlx::query!(
            r#"UPDATE smart_order_executions
               SET status = $2::sor_status,
                   realized_slippage_bps = $3,
                   completed_at = CASE WHEN $2 IN ('completed','failed','rolled_back')
                                       THEN NOW() ELSE NULL END
               WHERE execution_id = $1"#,
            execution_id,
            status as SorStatus,
            slippage_bps.map(|v| sqlx::types::BigDecimal::try_from(v).ok()).flatten(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Child orders ──────────────────────────────────────────────────────────

    pub async fn insert_child_order(&self, child: &SorChildOrder) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO sor_child_orders
               (child_order_id, execution_id, venue_id, allocation_pct,
                allocated_amount, status)
               VALUES ($1,$2,$3,$4,$5,$6::child_order_status)"#,
            child.child_order_id,
            child.execution_id,
            child.venue_id,
            child.allocation_pct,
            child.allocated_amount,
            child.status.clone() as ChildOrderStatus,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_child_order_filled(
        &self,
        child_order_id: Uuid,
        filled_amount: &sqlx::types::BigDecimal,
        slippage_bps: f64,
        venue_order_ref: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"UPDATE sor_child_orders
               SET filled_amount = $2,
                   slippage_bps  = $3,
                   venue_order_ref = $4,
                   status = 'filled',
                   filled_at = NOW()
               WHERE child_order_id = $1"#,
            child_order_id,
            filled_amount,
            sqlx::types::BigDecimal::try_from(slippage_bps).unwrap_or_default(),
            venue_order_ref,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn fail_child_order(&self, child_order_id: Uuid, reason: &str) -> Result<()> {
        sqlx::query!(
            r#"UPDATE sor_child_orders
               SET status = 'failed', failed_reason = $2
               WHERE child_order_id = $1"#,
            child_order_id,
            reason,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Rebalancing rules ─────────────────────────────────────────────────────

    pub async fn list_enabled_rules(&self) -> Result<Vec<TreasuryRebalancingRule>> {
        let rows = sqlx::query_as!(
            TreasuryRebalancingRule,
            r#"SELECT rule_id, currency_code,
                      min_inventory_pct, target_inventory_pct, max_inventory_pct,
                      trigger_type AS "trigger_type: RebalancingTrigger",
                      schedule_cron, enabled, last_triggered_at,
                      created_at, updated_at
               FROM treasury_rebalancing_rules
               WHERE enabled = TRUE"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn record_rebalance_log(
        &self,
        rule_id: Uuid,
        currency_code: &str,
        trigger: RebalancingTrigger,
        amount: &sqlx::types::BigDecimal,
        status: RebalanceStatus,
        stellar_tx_hash: Option<&str>,
        lock_key: &str,
        error: Option<&str>,
    ) -> Result<Uuid> {
        let log_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO treasury_rebalancing_log
               (log_id, rule_id, currency_code, trigger_type, amount_rebalanced,
                status, stellar_tx_hash, redis_lock_key, error_message,
                completed_at)
               VALUES ($1,$2,$3,$4::rebalancing_trigger,$5,$6::rebalance_status,$7,$8,$9,
                       CASE WHEN $6 IN ('completed','failed') THEN NOW() ELSE NULL END)"#,
            log_id,
            rule_id,
            currency_code,
            trigger as RebalancingTrigger,
            amount,
            status as RebalanceStatus,
            stellar_tx_hash,
            lock_key,
            error,
        )
        .execute(&self.pool)
        .await?;
        Ok(log_id)
    }

    pub async fn touch_rule_triggered(&self, rule_id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE treasury_rebalancing_rules
             SET last_triggered_at = $1, updated_at = $1
             WHERE rule_id = $2",
            Utc::now(),
            rule_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
