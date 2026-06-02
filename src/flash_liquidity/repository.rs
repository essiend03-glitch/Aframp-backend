//! #488 Flash Liquidity — database repository.

use super::models::*;
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

pub struct FlashLiquidityRepository {
    pool: PgPool,
}

impl FlashLiquidityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_active_facilities(&self) -> Result<Vec<CreditFacility>> {
        let rows = sqlx::query_as!(
            CreditFacility,
            r#"SELECT facility_id, lender_name, lender_api_endpoint,
                      max_drawdown_amount, current_utilization,
                      interest_rate_bps_daily, required_dcr,
                      collateral_asset,
                      status AS "status: FacilityStatus",
                      created_at, updated_at
               FROM credit_facilities
               WHERE status = 'active'
               ORDER BY interest_rate_bps_daily ASC"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn insert_draw(&self, draw: &FlashLiquidityDraw) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO flash_liquidity_draws
               (draw_id, facility_id, parent_settlement_id, corridor,
                draw_amount, collateral_amount, collateral_asset,
                escrow_account_hash, lock_xdr_signature, status,
                repayment_due_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10::draw_status,$11)"#,
            draw.draw_id,
            draw.facility_id,
            draw.parent_settlement_id,
            draw.corridor,
            draw.draw_amount,
            draw.collateral_amount,
            draw.collateral_asset,
            draw.escrow_account_hash,
            draw.lock_xdr_signature,
            draw.status.clone() as DrawStatus,
            draw.repayment_due_at,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_draw_status(
        &self,
        draw_id: Uuid,
        status: DrawStatus,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"UPDATE flash_liquidity_draws
               SET status = $2::draw_status,
                   error_message = $3,
                   repaid_at = CASE WHEN $2 = 'repaid' THEN NOW() ELSE NULL END,
                   updated_at = NOW()
               WHERE draw_id = $1"#,
            draw_id,
            status as DrawStatus,
            error,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_draw_escrow(
        &self,
        draw_id: Uuid,
        escrow_hash: &str,
        xdr_sig: &str,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE flash_liquidity_draws
             SET escrow_account_hash = $2, lock_xdr_signature = $3,
                 status = 'collateral_locked', updated_at = NOW()
             WHERE draw_id = $1",
            draw_id,
            escrow_hash,
            xdr_sig,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_pending_repayments(&self) -> Result<Vec<FlashLiquidityDraw>> {
        let rows = sqlx::query_as!(
            FlashLiquidityDraw,
            r#"SELECT draw_id, facility_id, parent_settlement_id, corridor,
                      draw_amount, collateral_amount, collateral_asset,
                      escrow_account_hash, lock_xdr_signature,
                      status AS "status: DrawStatus",
                      repayment_due_at, repaid_at, interest_accrued,
                      error_message, created_at, updated_at
               FROM flash_liquidity_draws
               WHERE status = 'disbursed'
                 AND repayment_due_at <= NOW() + INTERVAL '5 minutes'"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn insert_health_log(&self, log: &CollateralHealthLog) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO collateral_health_logs
               (log_id, draw_id, collateral_value_usd, debt_amount_usd,
                health_factor, near_liquidation, circuit_breaker_action)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
            log.log_id,
            log.draw_id,
            log.collateral_value_usd,
            log.debt_amount_usd,
            log.health_factor,
            log.near_liquidation,
            log.circuit_breaker_action,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_facility_utilization(
        &self,
        facility_id: Uuid,
        delta: &sqlx::types::BigDecimal,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE credit_facilities
             SET current_utilization = current_utilization + $2, updated_at = NOW()
             WHERE facility_id = $1",
            facility_id,
            delta,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
