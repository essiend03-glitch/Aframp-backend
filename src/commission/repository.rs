//! Database access layer for commission management engine.

use sqlx::{types::BigDecimal, PgPool, Postgres, Transaction};
use std::str::FromStr;
use uuid::Uuid;

use crate::database::error::DatabaseError;

use super::models::{
    CommissionBalance, CommissionStructure, CreateCommissionStructureInput, LedgerDirection,
    LedgerEntry, ManualAdjustmentInput, PayoutRecord, PayoutStatus,
};

#[derive(Clone)]
pub struct CommissionRepository {
    pool: PgPool,
}

impl CommissionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // commission_structures
    // -----------------------------------------------------------------------

    pub async fn create_structure(
        &self,
        input: &CreateCommissionStructureInput,
    ) -> Result<CommissionStructure, DatabaseError> {
        let tiers_json = input
            .tiers
            .as_ref()
            .map(|t| serde_json::to_value(t).unwrap());
        let pct = input
            .percentage_rate
            .map(|r| BigDecimal::from_str(&format!("{r:.7}")).unwrap());

        let row = sqlx::query_as::<_, CommissionStructure>(
            r#"
            INSERT INTO commission_structures
                (partner_id, name, commission_type, percentage_rate, fixed_stroops, tiers,
                 min_volume_stroops, max_volume_stroops, corridor,
                 effective_from, effective_to, created_by)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,
                    COALESCE($10, NOW()), $11, $12)
            RETURNING *
            "#,
        )
        .bind(input.partner_id)
        .bind(&input.name)
        .bind(&input.commission_type)
        .bind(pct)
        .bind(input.fixed_stroops)
        .bind(tiers_json)
        .bind(input.min_volume_stroops.unwrap_or(0))
        .bind(input.max_volume_stroops)
        .bind(&input.corridor)
        .bind(input.effective_from)
        .bind(input.effective_to)
        .bind(input.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from)?;

        Ok(row)
    }

    /// Fetch all active structures for a corridor (or corridor-agnostic ones).
    pub async fn active_structures(
        &self,
        corridor: Option<&str>,
        gross_fee_stroops: i64,
    ) -> Result<Vec<CommissionStructure>, sqlx::Error> {
        sqlx::query_as::<_, CommissionStructure>(
            r#"
            SELECT * FROM commission_structures
            WHERE is_active = TRUE
              AND NOW() BETWEEN effective_from AND COALESCE(effective_to, 'infinity')
              AND (corridor IS NULL OR corridor = $1)
              AND $2 >= min_volume_stroops
              AND ($3 <= max_volume_stroops OR max_volume_stroops IS NULL)
            ORDER BY partner_id, effective_from
            "#,
        )
        .bind(corridor)
        .bind(gross_fee_stroops)
        .bind(gross_fee_stroops)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn structure_by_partner(
        &self,
        partner_id: Uuid,
    ) -> Result<Vec<CommissionStructure>, sqlx::Error> {
        sqlx::query_as::<_, CommissionStructure>(
            "SELECT * FROM commission_structures WHERE partner_id = $1 ORDER BY created_at DESC",
        )
        .bind(partner_id)
        .fetch_all(&self.pool)
        .await
    }

    // -----------------------------------------------------------------------
    // partner_revenue_ledger — writes inside a transaction
    // -----------------------------------------------------------------------

    /// Insert a ledger entry within the caller's transaction.
    /// The caller must also update `partner_commission_balances` atomically.
    pub async fn insert_ledger_entry_tx(
        tx: &mut Transaction<'_, Postgres>,
        partner_id: Uuid,
        transaction_id: Uuid,
        structure_id: Option<Uuid>,
        amount_stroops: i64,
        direction: &LedgerDirection,
        balance_after: i64,
        gross_fee_stroops: i64,
        platform_share_stroops: i64,
        tier_index: Option<i16>,
        corridor: Option<&str>,
        narrative: &str,
    ) -> Result<LedgerEntry, sqlx::Error> {
        sqlx::query_as::<_, LedgerEntry>(
            r#"
            INSERT INTO partner_revenue_ledger
                (partner_id, transaction_id, commission_structure_id,
                 amount_stroops, direction, balance_after_stroops,
                 gross_fee_stroops, platform_share_stroops,
                 tier_index, corridor, narrative)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            RETURNING *
            "#,
        )
        .bind(partner_id)
        .bind(transaction_id)
        .bind(structure_id)
        .bind(amount_stroops)
        .bind(direction)
        .bind(balance_after)
        .bind(gross_fee_stroops)
        .bind(platform_share_stroops)
        .bind(tier_index)
        .bind(corridor)
        .bind(narrative)
        .fetch_one(&mut **tx)
        .await
    }

    /// Upsert the partner's materialised balance within a transaction.
    pub async fn upsert_balance_tx(
        tx: &mut Transaction<'_, Postgres>,
        partner_id: Uuid,
        delta_stroops: i64,
        direction: &LedgerDirection,
        entry_id: Uuid,
    ) -> Result<CommissionBalance, sqlx::Error> {
        let (accrued_delta, paid_delta) = match direction {
            LedgerDirection::Credit => (delta_stroops, 0_i64),
            LedgerDirection::Debit => (0_i64, delta_stroops),
        };
        sqlx::query_as::<_, CommissionBalance>(
            r#"
            INSERT INTO partner_commission_balances
                (partner_id, accrued_stroops, paid_stroops, last_entry_id, updated_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (partner_id) DO UPDATE SET
                accrued_stroops = partner_commission_balances.accrued_stroops + $2,
                paid_stroops    = partner_commission_balances.paid_stroops    + $3,
                last_entry_id   = EXCLUDED.last_entry_id,
                updated_at      = NOW()
            RETURNING *
            "#,
        )
        .bind(partner_id)
        .bind(accrued_delta)
        .bind(paid_delta)
        .bind(entry_id)
        .fetch_one(&mut **tx)
        .await
    }

    pub async fn balance_for_partner(
        &self,
        partner_id: Uuid,
    ) -> Result<Option<CommissionBalance>, sqlx::Error> {
        sqlx::query_as::<_, CommissionBalance>(
            "SELECT * FROM partner_commission_balances WHERE partner_id = $1",
        )
        .bind(partner_id)
        .fetch_optional(&self.pool)
        .await
    }

    // -----------------------------------------------------------------------
    // Revenue statement queries
    // -----------------------------------------------------------------------

    pub async fn ledger_entries_for_partner(
        &self,
        partner_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<LedgerEntry>, sqlx::Error> {
        sqlx::query_as::<_, LedgerEntry>(
            r#"
            SELECT * FROM partner_revenue_ledger
            WHERE partner_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(partner_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    // -----------------------------------------------------------------------
    // Payout records
    // -----------------------------------------------------------------------

    pub async fn create_payout_record(
        &self,
        partner_id: Uuid,
        payout_address: &str,
        total_stroops: i64,
        entry_count: i32,
        batch_ref: &str,
        initiated_by: Uuid,
    ) -> Result<PayoutRecord, sqlx::Error> {
        sqlx::query_as::<_, PayoutRecord>(
            r#"
            INSERT INTO commission_payout_records
                (partner_id, payout_address, total_stroops, entry_count, batch_ref, initiated_by)
            VALUES ($1,$2,$3,$4,$5,$6)
            RETURNING *
            "#,
        )
        .bind(partner_id)
        .bind(payout_address)
        .bind(total_stroops)
        .bind(entry_count)
        .bind(batch_ref)
        .bind(initiated_by)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_payout_status(
        &self,
        id: Uuid,
        status: &PayoutStatus,
        stellar_tx_hash: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE commission_payout_records SET
                status          = $2,
                stellar_tx_hash = COALESCE($3, stellar_tx_hash),
                error_message   = $4,
                attempted_at    = COALESCE(attempted_at, NOW()),
                completed_at    = CASE WHEN $2 = 'completed' THEN NOW() ELSE completed_at END
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(stellar_tx_hash)
        .bind(error_message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch pending payout IDs for batch dispatch.
    pub async fn pending_payouts(&self) -> Result<Vec<PayoutRecord>, sqlx::Error> {
        sqlx::query_as::<_, PayoutRecord>(
            "SELECT * FROM commission_payout_records WHERE status = 'pending' ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn payouts_for_partner(
        &self,
        partner_id: Uuid,
    ) -> Result<Vec<PayoutRecord>, sqlx::Error> {
        sqlx::query_as::<_, PayoutRecord>(
            "SELECT * FROM commission_payout_records WHERE partner_id = $1 ORDER BY created_at DESC",
        )
        .bind(partner_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Mark all unpaid ledger entries for a partner as belonging to a payout.
    pub async fn link_entries_to_payout_tx(
        tx: &mut Transaction<'_, Postgres>,
        partner_id: Uuid,
        payout_id: Uuid,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE partner_revenue_ledger
            SET payout_record_id = $2
            WHERE partner_id = $1
              AND direction = 'credit'
              AND payout_record_id IS NULL
            "#,
        )
        .bind(partner_id)
        .bind(payout_id)
        .execute(&mut **tx)
        .await?;
        Ok(result.rows_affected() as i64)
    }

    /// Sum of unpaid credits for a partner.
    pub async fn unpaid_balance(&self, partner_id: Uuid) -> Result<i64, sqlx::Error> {
        let row: (Option<i64>,) = sqlx::query_as(
            r#"
            SELECT SUM(amount_stroops)
            FROM partner_revenue_ledger
            WHERE partner_id = $1
              AND direction = 'credit'
              AND payout_record_id IS NULL
            "#,
        )
        .bind(partner_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0.unwrap_or(0))
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
