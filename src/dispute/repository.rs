//! Database repository for Merchant Dispute Resolution (Issue #337).

use crate::dispute::models::*;
use crate::error::Error;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;

pub struct DisputeRepository {
    pool: PgPool,
}

impl DisputeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -------------------------------------------------------------------------
    // Disputes
    // -------------------------------------------------------------------------

    pub async fn create_dispute(
        &self,
        transaction_id: Uuid,
        customer_wallet: &str,
        merchant_id: Uuid,
        reason: DisputeReason,
        description: &str,
        transaction_amount: sqlx::types::BigDecimal,
        claimed_amount: sqlx::types::BigDecimal,
        escrow_active: bool,
        escrow_hold_pct: Option<sqlx::types::BigDecimal>,
    ) -> Result<Dispute, Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        // Merchant has 48 hours to respond.
        let deadline = now + chrono::Duration::hours(48);

        let dispute = sqlx::query_as::<_, Dispute>(
            r#"
            INSERT INTO disputes (
                id, transaction_id, customer_wallet, merchant_id, reason,
                description, status, transaction_amount, claimed_amount,
                merchant_response_deadline, escrow_active, escrow_hold_pct,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, 'open', $7, $8,
                $9, $10, $11,
                $12, $12
            )
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(transaction_id)
        .bind(customer_wallet)
        .bind(merchant_id)
        .bind(reason)
        .bind(description)
        .bind(transaction_amount)
        .bind(claimed_amount)
        .bind(deadline)
        .bind(escrow_active)
        .bind(escrow_hold_pct)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(dispute)
    }

    pub async fn get_dispute(&self, id: Uuid) -> Result<Option<Dispute>, Error> {
        sqlx::query_as::<_, Dispute>("SELECT * FROM disputes WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    pub async fn list_disputes_for_customer(
        &self,
        customer_wallet: &str,
        query: &DisputeListQuery,
    ) -> Result<DisputePage, Error> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM disputes WHERE customer_wallet = $1 AND ($2::dispute_status IS NULL OR status = $2)",
        )
        .bind(customer_wallet)
        .bind(query.status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        let disputes = sqlx::query_as::<_, Dispute>(
            r#"
            SELECT * FROM disputes
            WHERE customer_wallet = $1
              AND ($2::dispute_status IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(customer_wallet)
        .bind(query.status)
        .bind(query.page_size())
        .bind(query.offset())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(DisputePage {
            disputes,
            total,
            page: query.page(),
            page_size: query.page_size(),
        })
    }

    pub async fn list_disputes_for_merchant(
        &self,
        merchant_id: Uuid,
        query: &DisputeListQuery,
    ) -> Result<DisputePage, Error> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM disputes WHERE merchant_id = $1 AND ($2::dispute_status IS NULL OR status = $2)",
        )
        .bind(merchant_id)
        .bind(query.status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        let disputes = sqlx::query_as::<_, Dispute>(
            r#"
            SELECT * FROM disputes
            WHERE merchant_id = $1
              AND ($2::dispute_status IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(merchant_id)
        .bind(query.status)
        .bind(query.page_size())
        .bind(query.offset())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(DisputePage {
            disputes,
            total,
            page: query.page(),
            page_size: query.page_size(),
        })
    }

    pub async fn update_dispute_status(
        &self,
        id: Uuid,
        status: DisputeStatus,
    ) -> Result<Dispute, Error> {
        sqlx::query_as::<_, Dispute>(
            r#"
            UPDATE disputes
            SET status = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    pub async fn record_merchant_response(
        &self,
        id: Uuid,
        proposal: Option<serde_json::Value>,
    ) -> Result<Dispute, Error> {
        sqlx::query_as::<_, Dispute>(
            r#"
            UPDATE disputes
            SET status = 'under_review',
                merchant_responded_at = NOW(),
                settlement_proposal = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(proposal)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    pub async fn resolve_dispute(
        &self,
        id: Uuid,
        decision: DisputeDecision,
        refunded_amount: Option<sqlx::types::BigDecimal>,
        refund_tx_hash: Option<&str>,
        status: DisputeStatus,
    ) -> Result<Dispute, Error> {
        sqlx::query_as::<_, Dispute>(
            r#"
            UPDATE disputes
            SET status = $2,
                final_decision = $3,
                refunded_amount = $4,
                refund_tx_hash = $5,
                escrow_active = false,
                resolved_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(decision)
        .bind(refunded_amount)
        .bind(refund_tx_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    // -------------------------------------------------------------------------
    // Evidence
    // -------------------------------------------------------------------------

    pub async fn add_evidence(
        &self,
        dispute_id: Uuid,
        submitter: EvidenceSubmitter,
        submitter_id: &str,
        label: &str,
        file_url: Option<&str>,
        notes: Option<&str>,
        delivery_status: Option<&str>,
    ) -> Result<DisputeEvidence, Error> {
        sqlx::query_as::<_, DisputeEvidence>(
            r#"
            INSERT INTO dispute_evidence (
                id, dispute_id, submitter, submitter_id, label,
                file_url, notes, delivery_status, created_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, NOW()
            )
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(dispute_id)
        .bind(submitter)
        .bind(submitter_id)
        .bind(label)
        .bind(file_url)
        .bind(notes)
        .bind(delivery_status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    pub async fn list_evidence(&self, dispute_id: Uuid) -> Result<Vec<DisputeEvidence>, Error> {
        sqlx::query_as::<_, DisputeEvidence>(
            "SELECT * FROM dispute_evidence WHERE dispute_id = $1 ORDER BY created_at ASC",
        )
        .bind(dispute_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    // -------------------------------------------------------------------------
    // Audit log
    // -------------------------------------------------------------------------

    pub async fn append_audit_log(
        &self,
        dispute_id: Uuid,
        actor: &str,
        action: &str,
        previous_status: Option<DisputeStatus>,
        new_status: Option<DisputeStatus>,
        notes: Option<&str>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO dispute_audit_log (
                id, dispute_id, actor, action,
                previous_status, new_status, notes, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(dispute_id)
        .bind(actor)
        .bind(action)
        .bind(previous_status)
        .bind(new_status)
        .bind(notes)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn get_audit_log(
        &self,
        dispute_id: Uuid,
    ) -> Result<Vec<DisputeAuditLog>, Error> {
        sqlx::query_as::<_, DisputeAuditLog>(
            "SELECT * FROM dispute_audit_log WHERE dispute_id = $1 ORDER BY created_at ASC",
        )
        .bind(dispute_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    // -------------------------------------------------------------------------
    // Escrow / deadline helpers
    // -------------------------------------------------------------------------

    /// Flag disputes whose 48-hour merchant response window has expired.
    pub async fn flag_overdue_disputes(&self) -> Result<Vec<Uuid>, Error> {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE disputes
            SET status = 'mediation', updated_at = NOW()
            WHERE status = 'open'
              AND merchant_response_deadline < NOW()
            RETURNING id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
        Ok(ids)
    }
}
