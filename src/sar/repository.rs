//! SAR database repository

use sqlx::PgPool;
use uuid::Uuid;

use super::models::{SarAuditEntry, SarReport};

pub struct SarRepository {
    pool: PgPool,
}

impl SarRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, report: &SarReport) -> Result<SarReport, anyhow::Error> {
        let r = sqlx::query_as!(
            SarReport,
            r#"
            INSERT INTO sar_reports
                (id, aml_case_id, transaction_id, wallet_address, status, authority,
                 activity_snapshot, rendered_report, created_at, updated_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)
            RETURNING *
            "#,
            report.id,
            report.aml_case_id,
            report.transaction_id,
            report.wallet_address,
            report.status,
            report.authority,
            report.activity_snapshot,
            report.rendered_report,
            report.created_at,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(r)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<SarReport>, anyhow::Error> {
        Ok(sqlx::query_as!(SarReport, "SELECT * FROM sar_reports WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn list_by_status(&self, status: &str) -> Result<Vec<SarReport>, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarReport,
            "SELECT * FROM sar_reports WHERE status = $1 ORDER BY created_at DESC",
            status
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Transition status and record the audit entry atomically.
    pub async fn transition(
        &self,
        id: Uuid,
        to_status: &str,
        officer_id: &str,
        notes: Option<&str>,
        amended_report: Option<&str>,
    ) -> Result<SarReport, anyhow::Error> {
        let mut tx = self.pool.begin().await?;

        // Fetch current status for audit log
        let current: (String,) =
            sqlx::query_as("SELECT status FROM sar_reports WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;

        // Update the report
        let report = sqlx::query_as!(
            SarReport,
            r#"
            UPDATE sar_reports
            SET status = $2,
                reviewed_by = COALESCE($3, reviewed_by),
                review_notes = COALESCE($4, review_notes),
                rendered_report = COALESCE($5, rendered_report),
                filed_at = CASE WHEN $2 = 'Filed' THEN NOW() ELSE filed_at END,
                acknowledged_at = CASE WHEN $2 = 'Acknowledged' THEN NOW() ELSE acknowledged_at END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
            id,
            to_status,
            officer_id,
            notes,
            amended_report,
        )
        .fetch_one(&mut *tx)
        .await?;

        // Append immutable audit entry
        sqlx::query!(
            r#"
            INSERT INTO sar_audit_log (id, sar_id, actor_id, action, from_status, to_status, notes, created_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,NOW())
            "#,
            Uuid::new_v4(),
            id,
            officer_id,
            format!("transition_to_{}", to_status.to_lowercase()),
            current.0,
            to_status,
            notes,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(report)
    }

    pub async fn get_audit_log(&self, sar_id: Uuid) -> Result<Vec<SarAuditEntry>, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarAuditEntry,
            "SELECT * FROM sar_audit_log WHERE sar_id = $1 ORDER BY created_at ASC",
            sar_id
        )
        .fetch_all(&self.pool)
        .await?)
    }
}
