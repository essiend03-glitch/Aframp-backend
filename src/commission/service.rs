//! Atomic transactional ledger service — records split-fee commissions and
//! manual adjustments within a single isolated DB transaction (Issue #471).

use std::sync::Arc;

use chrono::Utc;
use sqlx::PgPool;
use tracing::{error, info, instrument};
use uuid::Uuid;

use super::{
    metrics,
    models::{
        CommissionStructure, CreateCommissionStructureInput, LedgerDirection, LedgerEntry,
        ManualAdjustmentInput, PayoutRecord, RevenueStatement,
    },
    repository::CommissionRepository,
    split_fee::{CommissionBreakdown, SplitFeeEngine},
};
use crate::database::error::DatabaseError;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum CommissionError {
    #[error("split-fee error: {0}")]
    SplitFee(#[from] super::split_fee::SplitFeeError),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("invariant violation: gross {gross} != platform {platform} + partner {partner}")]
    InvariantViolation { gross: i64, platform: i64, partner: i64 },
    #[error("partner not found: {0}")]
    PartnerNotFound(Uuid),
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

pub struct CommissionService {
    repo: Arc<CommissionRepository>,
    engine: Arc<SplitFeeEngine>,
    pool: PgPool,
}

impl CommissionService {
    pub fn new(pool: PgPool) -> Self {
        let repo = Arc::new(CommissionRepository::new(pool.clone()));
        let engine = Arc::new(SplitFeeEngine::new(Arc::clone(&repo)));
        Self { repo, engine, pool }
    }

    pub fn repo(&self) -> &Arc<CommissionRepository> {
        &self.repo
    }

    // -----------------------------------------------------------------------
    // Configuration
    // -----------------------------------------------------------------------

    pub async fn configure_structure(
        &self,
        input: CreateCommissionStructureInput,
    ) -> Result<CommissionStructure, CommissionError> {
        let s = self.repo.create_structure(&input).await.map_err(CommissionError::Db)?;
        info!(
            structure_id = %s.id,
            partner_id = %s.partner_id,
            commission_type = ?s.commission_type,
            "commission structure created"
        );
        Ok(s)
    }

    // -----------------------------------------------------------------------
    // Core: record commissions for a transaction (called concurrently)
    // -----------------------------------------------------------------------

    /// Evaluates the split for `gross_fee_stroops` and writes all partner
    /// ledger credits atomically. Returns the ledger entries created.
    #[instrument(skip(self), fields(transaction_id = %transaction_id, gross_fee_stroops))]
    pub async fn record_transaction_commissions(
        &self,
        transaction_id: Uuid,
        gross_fee_stroops: i64,
        corridor: Option<&str>,
        cumulative_volume_stroops: i64,
    ) -> Result<Vec<LedgerEntry>, CommissionError> {
        let breakdown = self
            .engine
            .evaluate(gross_fee_stroops, corridor, cumulative_volume_stroops)
            .await?;

        self.persist_breakdown(transaction_id, &breakdown, corridor).await
    }

    /// Persist a pre-computed breakdown inside a single DB transaction.
    async fn persist_breakdown(
        &self,
        transaction_id: Uuid,
        breakdown: &CommissionBreakdown,
        corridor: Option<&str>,
    ) -> Result<Vec<LedgerEntry>, CommissionError> {
        let mut tx = self.pool.begin().await?;
        let mut entries = Vec::with_capacity(breakdown.partner_splits.len());

        for split in &breakdown.partner_splits {
            // Fetch current balance to compute running balance_after
            let current_balance = sqlx::query_scalar::<_, i64>(
                "SELECT COALESCE(accrued_stroops, 0) - COALESCE(paid_stroops, 0)
                 FROM partner_commission_balances WHERE partner_id = $1",
            )
            .bind(split.partner_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(0);

            let balance_after = current_balance + split.commission_stroops;

            let entry = CommissionRepository::insert_ledger_entry_tx(
                &mut tx,
                split.partner_id,
                transaction_id,
                Some(split.structure_id),
                split.commission_stroops,
                &LedgerDirection::Credit,
                balance_after,
                breakdown.gross_fee_stroops,
                breakdown.platform_share_stroops,
                split.tier_index,
                corridor,
                &format!(
                    "Commission credit for tx {} — tier {:?}",
                    transaction_id, split.tier_index
                ),
            )
            .await?;

            CommissionRepository::upsert_balance_tx(
                &mut tx,
                split.partner_id,
                split.commission_stroops,
                &LedgerDirection::Credit,
                entry.entry_id,
            )
            .await?;

            metrics::commission_accrued(
                &split.partner_id.to_string(),
                corridor.unwrap_or("all"),
                split.commission_stroops,
            );

            info!(
                entry_id = %entry.entry_id,
                partner_id = %split.partner_id,
                amount_stroops = split.commission_stroops,
                gross_fee_stroops = breakdown.gross_fee_stroops,
                platform_share_stroops = breakdown.platform_share_stroops,
                tier_index = ?split.tier_index,
                corridor,
                "commission ledger entry created"
            );

            entries.push(entry);
        }

        tx.commit().await?;
        Ok(entries)
    }

    // -----------------------------------------------------------------------
    // Manual adjustment (admin override)
    // -----------------------------------------------------------------------

    #[instrument(skip(self), fields(partner_id = %input.partner_id))]
    pub async fn manual_adjustment(
        &self,
        input: ManualAdjustmentInput,
    ) -> Result<LedgerEntry, CommissionError> {
        // Validate invariant before writing
        let partner_total = match input.direction {
            LedgerDirection::Credit => input.amount_stroops,
            LedgerDirection::Debit => 0,
        };
        if input.gross_fee_stroops != input.platform_share_stroops + partner_total
            && input.direction == LedgerDirection::Credit
        {
            metrics::invariant_violation();
            return Err(CommissionError::InvariantViolation {
                gross: input.gross_fee_stroops,
                platform: input.platform_share_stroops,
                partner: partner_total,
            });
        }

        let mut tx = self.pool.begin().await?;

        let current_balance = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(accrued_stroops, 0) - COALESCE(paid_stroops, 0)
             FROM partner_commission_balances WHERE partner_id = $1",
        )
        .bind(input.partner_id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let balance_after = match input.direction {
            LedgerDirection::Credit => current_balance + input.amount_stroops,
            LedgerDirection::Debit => current_balance - input.amount_stroops,
        };

        let entry = CommissionRepository::insert_ledger_entry_tx(
            &mut tx,
            input.partner_id,
            input.transaction_id,
            None,
            input.amount_stroops,
            &input.direction,
            balance_after,
            input.gross_fee_stroops,
            input.platform_share_stroops,
            None,
            None,
            &format!(
                "[MANUAL ADJUSTMENT by {}] {}",
                input.initiated_by, input.narrative
            ),
        )
        .await?;

        CommissionRepository::upsert_balance_tx(
            &mut tx,
            input.partner_id,
            input.amount_stroops,
            &input.direction,
            entry.entry_id,
        )
        .await?;

        tx.commit().await?;

        info!(
            entry_id = %entry.entry_id,
            partner_id = %input.partner_id,
            amount_stroops = input.amount_stroops,
            direction = ?input.direction,
            initiated_by = %input.initiated_by,
            narrative = input.narrative,
            "manual ledger adjustment committed"
        );

        Ok(entry)
    }

    // -----------------------------------------------------------------------
    // Revenue statement
    // -----------------------------------------------------------------------

    pub async fn revenue_statement(
        &self,
        partner_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<RevenueStatement, CommissionError> {
        let balance = self
            .repo
            .balance_for_partner(partner_id)
            .await?
            .unwrap_or_else(|| super::models::CommissionBalance {
                partner_id,
                accrued_stroops: 0,
                paid_stroops: 0,
                last_entry_id: None,
                updated_at: Utc::now(),
            });

        let entries = self
            .repo
            .ledger_entries_for_partner(partner_id, limit, offset)
            .await?;

        let payouts = self.repo.payouts_for_partner(partner_id).await?;

        let unpaid = balance.accrued_stroops - balance.paid_stroops;
        metrics::set_accrued_liability(&partner_id.to_string(), unpaid.max(0));

        Ok(RevenueStatement {
            partner_id,
            accrued_stroops: balance.accrued_stroops,
            paid_stroops: balance.paid_stroops,
            unpaid_stroops: unpaid.max(0),
            entries,
            payouts,
            generated_at: Utc::now(),
        })
    }
}
