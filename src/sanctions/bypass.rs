//! Dual-authorisation bypass — #419
//!
//! An engineer can request a bypass for a blocked transaction only if **two
//! distinct** approvers sign off.  The first approver opens the request; the
//! second approver (who must be a different user) closes it.  Both actions are
//! written to `bypass_audit` for regulator review.

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::BypassRequest;

pub struct BypassService {
    pool: PgPool,
}

#[derive(Debug, thiserror::Error)]
pub enum BypassError {
    #[error("bypass already approved")]
    AlreadyApproved,
    #[error("second approver must differ from first approver")]
    SameApprover,
    #[error("bypass request not found")]
    NotFound,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl BypassService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// First approver opens a bypass request.
    pub async fn request_bypass(
        &self,
        transaction_id: Uuid,
        first_approver_id: &str,
        reason: &str,
    ) -> Result<BypassRequest, BypassError> {
        let req = sqlx::query_as!(
            BypassRequest,
            r#"
            INSERT INTO bypass_audit
                (id, transaction_id, reason, first_approver_id, approved, created_at)
            VALUES ($1, $2, $3, $4, false, $5)
            RETURNING
                id, transaction_id, reason, first_approver_id,
                second_approver_id, approved, created_at, approved_at
            "#,
            Uuid::new_v4(),
            transaction_id,
            reason,
            first_approver_id,
            Utc::now(),
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(req)
    }

    /// Second approver completes the bypass.  Enforces that the two approvers
    /// are distinct individuals.
    pub async fn approve_bypass(
        &self,
        bypass_id: Uuid,
        second_approver_id: &str,
    ) -> Result<BypassRequest, BypassError> {
        // Fetch the pending request
        let existing = sqlx::query_as!(
            BypassRequest,
            r#"
            SELECT id, transaction_id, reason, first_approver_id,
                   second_approver_id, approved, created_at, approved_at
            FROM bypass_audit WHERE id = $1
            "#,
            bypass_id,
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(BypassError::NotFound)?;

        if existing.approved {
            return Err(BypassError::AlreadyApproved);
        }
        if existing.first_approver_id == second_approver_id {
            return Err(BypassError::SameApprover);
        }

        let updated = sqlx::query_as!(
            BypassRequest,
            r#"
            UPDATE bypass_audit
            SET second_approver_id = $2, approved = true, approved_at = $3
            WHERE id = $1
            RETURNING
                id, transaction_id, reason, first_approver_id,
                second_approver_id, approved, created_at, approved_at
            "#,
            bypass_id,
            second_approver_id,
            Utc::now(),
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    /// Check whether a transaction has an approved bypass.
    pub async fn is_bypassed(&self, transaction_id: Uuid) -> Result<bool, BypassError> {
        let row: Option<(bool,)> = sqlx::query_as(
            "SELECT approved FROM bypass_audit WHERE transaction_id = $1 AND approved = true LIMIT 1",
        )
        .bind(transaction_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.is_some())
    }
}
