//! Database repository for DR/BCP (Issue #DR-BCP).

use crate::dr_bcp::models::*;
use sqlx::PgPool;
use uuid::Uuid;

pub struct DrBcpRepository {
    pool: PgPool,
}

impl DrBcpRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // BIA
    // -----------------------------------------------------------------------

    pub async fn list_bia_entries(&self) -> Result<Vec<BiaEntry>, sqlx::Error> {
        sqlx::query_as!(
            BiaEntry,
            r#"SELECT id, service_name,
                      criticality AS "criticality: ServiceCriticality",
                      mtd_seconds, rpo_seconds, rto_seconds,
                      description, created_at, updated_at
               FROM dr_bia_entries ORDER BY criticality, service_name"#
        )
        .fetch_all(&self.pool)
        .await
    }

    // -----------------------------------------------------------------------
    // Backups
    // -----------------------------------------------------------------------

    pub async fn latest_backup(&self) -> Result<Option<BackupRecord>, sqlx::Error> {
        sqlx::query_as!(
            BackupRecord,
            r#"SELECT id, s3_key, s3_bucket, checksum_sha256,
                      size_bytes, verified, last_verified_at, created_at
               FROM dr_backup_records ORDER BY created_at DESC LIMIT 1"#
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn record_backup(&self, rec: &BackupRecord) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"INSERT INTO dr_backup_records
               (id, s3_key, s3_bucket, checksum_sha256, size_bytes, verified, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
            rec.id,
            rec.s3_key,
            rec.s3_bucket,
            rec.checksum_sha256,
            rec.size_bytes,
            rec.verified,
            rec.created_at,
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    pub async fn mark_backup_verified(&self, backup_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE dr_backup_records SET verified = true, last_verified_at = NOW() WHERE id = $1",
            backup_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    // -----------------------------------------------------------------------
    // Restore test runs
    // -----------------------------------------------------------------------

    pub async fn latest_restore_test(&self) -> Result<Option<RestoreTestRun>, sqlx::Error> {
        sqlx::query_as!(
            RestoreTestRun,
            r#"SELECT id, backup_id,
                      result AS "result: RestoreTestResult",
                      restore_duration_seconds,
                      rpo_achieved_seconds, rto_achieved_seconds,
                      error_message, run_at
               FROM dr_restore_test_runs ORDER BY run_at DESC LIMIT 1"#
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn record_restore_test(&self, run: &RestoreTestRun) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"INSERT INTO dr_restore_test_runs
               (id, backup_id, result, restore_duration_seconds,
                rpo_achieved_seconds, rto_achieved_seconds, error_message, run_at)
               VALUES ($1,$2,$3::restore_test_result,$4,$5,$6,$7,$8)"#,
            run.id,
            run.backup_id,
            run.result as RestoreTestResult,
            run.restore_duration_seconds,
            run.rpo_achieved_seconds,
            run.rto_achieved_seconds,
            run.error_message,
            run.run_at,
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    // -----------------------------------------------------------------------
    // Incidents
    // -----------------------------------------------------------------------

    pub async fn active_incidents(&self) -> Result<Vec<DrIncident>, sqlx::Error> {
        sqlx::query_as!(
            DrIncident,
            r#"SELECT id, title, description,
                      status AS "status: DrIncidentStatus",
                      commander_id, affected_services,
                      rpo_achieved_seconds, rto_achieved_seconds,
                      declared_at, resolved_at, created_at, updated_at
               FROM dr_incidents
               WHERE status NOT IN ('resolved','post_mortem_pending')
               ORDER BY declared_at DESC"#
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn create_incident(&self, inc: &DrIncident) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"INSERT INTO dr_incidents
               (id, title, description, status, commander_id, affected_services,
                declared_at, created_at, updated_at)
               VALUES ($1,$2,$3,$4::dr_incident_status,$5,$6,$7,$8,$9)"#,
            inc.id,
            inc.title,
            inc.description,
            inc.status as DrIncidentStatus,
            inc.commander_id,
            inc.affected_services,
            inc.declared_at,
            inc.created_at,
            inc.updated_at,
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    pub async fn update_incident_status(
        &self,
        id: Uuid,
        req: &UpdateIncidentStatusRequest,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE dr_incidents
               SET status = $2::dr_incident_status,
                   rpo_achieved_seconds = COALESCE($3, rpo_achieved_seconds),
                   rto_achieved_seconds = COALESCE($4, rto_achieved_seconds),
                   resolved_at = CASE WHEN $2 = 'resolved' THEN NOW() ELSE resolved_at END,
                   updated_at = NOW()
               WHERE id = $1"#,
            id,
            req.status as DrIncidentStatus,
            req.rpo_achieved_seconds,
            req.rto_achieved_seconds,
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    // -----------------------------------------------------------------------
    // Regulatory notifications
    // -----------------------------------------------------------------------

    pub async fn record_regulatory_notification(
        &self,
        notif: &RegulatoryNotification,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"INSERT INTO dr_regulatory_notifications
               (id, incident_id, body, template_used, sent_at)
               VALUES ($1,$2,$3::regulatory_body,$4,$5)"#,
            notif.id,
            notif.incident_id,
            notif.body as RegulatoryBody,
            notif.template_used,
            notif.sent_at,
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }
}
