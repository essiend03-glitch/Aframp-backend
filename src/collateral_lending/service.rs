use crate::collateral_lending::models::*;
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use sqlx::types::BigDecimal;
use std::str::FromStr;

pub struct CollateralLendingService {
    pool: PgPool,
}

impl CollateralLendingService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn open_position(&self, req: OpenPositionRequest) -> Result<LendingPosition> {
        let position_id = Uuid::new_v4();
        let now = Utc::now();
        let collateral_ratio = if req.borrowed_amount > BigDecimal::from(0) {
            req.collateral_amount.clone() / req.borrowed_amount.clone()
        } else {
            BigDecimal::from(0)
        };
        let health_factor = if req.liquidation_threshold_ratio > BigDecimal::from(0) {
            collateral_ratio.clone() / req.liquidation_threshold_ratio.clone()
        } else {
            BigDecimal::from(0)
        };

        let position = sqlx::query_as::<_, LendingPosition>(
            r#"INSERT INTO lending_positions (
                position_id, wallet_id, lending_protocol_id,
                collateral_asset_code, collateral_amount, collateral_value_fiat,
                borrowed_asset_code, borrowed_amount, borrowed_value_fiat,
                collateral_ratio, liquidation_threshold_ratio, health_factor,
                interest_rate, interest_accrued, status,
                opened_at, last_health_check_at, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)
            RETURNING *"#,
        )
        .bind(position_id)
        .bind(req.wallet_id)
        .bind(&req.lending_protocol_id)
        .bind(&req.collateral_asset_code)
        .bind(&req.collateral_amount)
        .bind(&req.collateral_amount) // collateral_value_fiat = collateral_amount initially
        .bind(&req.borrowed_asset_code)
        .bind(&req.borrowed_amount)
        .bind(&req.borrowed_amount) // borrowed_value_fiat = borrowed_amount initially
        .bind(&collateral_ratio)
        .bind(&req.liquidation_threshold_ratio)
        .bind(&health_factor)
        .bind(&req.interest_rate)
        .bind(BigDecimal::from(0))
        .bind(LendingPositionStatus::Active)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(position)
    }

    pub async fn repay_loan(&self, req: RepayLoanRequest) -> Result<LoanRepayment> {
        let position = self.get_position(req.position_id).await?;
        let interest_paid = position.interest_accrued.clone();
        let principal_repaid = req.repayment_amount.clone() - interest_paid.clone();
        let remaining_balance = if position.borrowed_amount > principal_repaid {
            position.borrowed_amount.clone() - principal_repaid.clone()
        } else {
            BigDecimal::from(0)
        };

        let repayment = sqlx::query_as::<_, LoanRepayment>(
            r#"INSERT INTO loan_repayments (
                repayment_id, position_id, repayment_amount, repayment_asset,
                interest_paid, principal_repaid, remaining_balance,
                transaction_reference, repaid_at, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            RETURNING *"#,
        )
        .bind(Uuid::new_v4())
        .bind(req.position_id)
        .bind(&req.repayment_amount)
        .bind(&req.repayment_asset)
        .bind(&interest_paid)
        .bind(&principal_repaid)
        .bind(&remaining_balance)
        .bind(&req.transaction_reference)
        .bind(Utc::now())
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await?;

        // Update position status if fully repaid
        if remaining_balance == BigDecimal::from(0) {
            sqlx::query("UPDATE lending_positions SET status = $1, updated_at = $2 WHERE position_id = $3")
                .bind(LendingPositionStatus::Repaid)
                .bind(Utc::now())
                .bind(req.position_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(repayment)
    }

    pub async fn adjust_collateral(&self, req: AdjustCollateralRequest) -> Result<CollateralAdjustment> {
        let position = self.get_position(req.position_id).await?;
        let pre_collateral = position.collateral_amount.clone();
        let pre_health = position.health_factor.clone();

        let post_collateral = match req.adjustment_type {
            CollateralAdjustmentType::Deposit => pre_collateral.clone() + req.adjustment_amount.clone(),
            CollateralAdjustmentType::Withdrawal => {
                if pre_collateral > req.adjustment_amount {
                    pre_collateral.clone() - req.adjustment_amount.clone()
                } else {
                    return Err(anyhow::anyhow!("Insufficient collateral for withdrawal"));
                }
            }
        };

        let post_health = if position.liquidation_threshold_ratio > BigDecimal::from(0) && position.borrowed_amount > BigDecimal::from(0) {
            let new_ratio = post_collateral.clone() / position.borrowed_amount.clone();
            new_ratio / position.liquidation_threshold_ratio.clone()
        } else {
            BigDecimal::from(0)
        };

        let adjustment = sqlx::query_as::<_, CollateralAdjustment>(
            r#"INSERT INTO collateral_adjustments (
                adjustment_id, position_id, adjustment_type, adjustment_amount,
                pre_adjustment_collateral, post_adjustment_collateral,
                pre_adjustment_health_factor, post_adjustment_health_factor,
                adjusted_at, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            RETURNING *"#,
        )
        .bind(Uuid::new_v4())
        .bind(req.position_id)
        .bind(&req.adjustment_type)
        .bind(&req.adjustment_amount)
        .bind(&pre_collateral)
        .bind(&post_collateral)
        .bind(&pre_health)
        .bind(&post_health)
        .bind(Utc::now())
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await?;

        // Update position collateral and health factor
        sqlx::query(
            "UPDATE lending_positions SET collateral_amount = $1, health_factor = $2, updated_at = $3 WHERE position_id = $4"
        )
        .bind(&post_collateral)
        .bind(&post_health)
        .bind(Utc::now())
        .bind(req.position_id)
        .execute(&self.pool)
        .await?;

        Ok(adjustment)
    }

    pub async fn get_position(&self, position_id: Uuid) -> Result<LendingPosition> {
        let position = sqlx::query_as::<_, LendingPosition>(
            "SELECT * FROM lending_positions WHERE position_id = $1"
        )
        .bind(position_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(position)
    }

    pub async fn list_positions_by_wallet(&self, wallet_id: Uuid) -> Result<Vec<LendingPosition>> {
        let positions = sqlx::query_as::<_, LendingPosition>(
            "SELECT * FROM lending_positions WHERE wallet_id = $1 ORDER BY created_at DESC"
        )
        .bind(wallet_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(positions)
    }

    /// Check health factors and mark at-risk positions
    pub async fn run_health_check(&self) -> Result<usize> {
        let threshold = BigDecimal::from_str("1.1").unwrap();
        let result = sqlx::query(
            r#"UPDATE lending_positions
               SET status = CASE WHEN health_factor < $1 THEN 'at_risk'::lending_position_status ELSE status END,
                   last_health_check_at = $2,
                   updated_at = $2
               WHERE status = 'active'"#,
        )
        .bind(&threshold)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as usize)
    }

    pub async fn record_liquidation(&self, event: LiquidationEvent) -> Result<LiquidationEvent> {
        let record = sqlx::query_as::<_, LiquidationEvent>(
            r#"INSERT INTO liquidation_events (
                liquidation_id, position_id, trigger_health_factor,
                liquidated_collateral_amount, liquidated_collateral_value,
                repaid_debt_amount, liquidation_penalty_amount,
                liquidator_address, liquidated_at, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            RETURNING *"#,
        )
        .bind(event.liquidation_id)
        .bind(event.position_id)
        .bind(&event.trigger_health_factor)
        .bind(&event.liquidated_collateral_amount)
        .bind(&event.liquidated_collateral_value)
        .bind(&event.repaid_debt_amount)
        .bind(&event.liquidation_penalty_amount)
        .bind(&event.liquidator_address)
        .bind(event.liquidated_at)
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await?;

        sqlx::query("UPDATE lending_positions SET status = $1, updated_at = $2 WHERE position_id = $3")
            .bind(LendingPositionStatus::Liquidated)
            .bind(Utc::now())
            .bind(event.position_id)
            .execute(&self.pool)
            .await?;

        Ok(record)
    }
}
