use crate::database::error::DatabaseError;
use crate::regulatory_evidence::models::*;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct RegulatoryEvidenceRepository {
    pool: PgPool,
}

impl RegulatoryEvidenceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Evidence packages ─────────────────────────────────────────────────────

    pub async fn insert_package(
        &self,
        pkg: &EvidencePackage,
    ) -> Result<EvidencePackageRecord, DatabaseError> {
        let row = sqlx::query_as!(
            EvidencePackageRecord,
            r#"INSERT INTO regulatory_evidence_packages
               (id, scope_label, period_from, period_to, generated_at, generated_by,
                checksum_sha256, signature_hmac_sha256,
                aml_log_count, travel_rule_count, kyc_event_count,
                multisig_event_count, policy_snapshot_count, system_test_count)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
               RETURNING *"#,
            pkg.id,
            pkg.scope_label,
            pkg.period_from,
            pkg.period_to,
            pkg.generated_at,
            pkg.generated_by,
            pkg.checksum_sha256,
            pkg.signature_hmac_sha256,
            pkg.aml_log_count,
            pkg.travel_rule_count,
            pkg.kyc_event_count,
            pkg.multisig_event_count,
            pkg.policy_snapshot_count,
            pkg.system_test_count,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(row)
    }

    pub async fn list_packages(
        &self,
        scope_label: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<EvidencePackageRecord>, DatabaseError> {
        let rows = sqlx::query_as!(
            EvidencePackageRecord,
            r#"SELECT * FROM regulatory_evidence_packages
               WHERE ($1::text IS NULL OR scope_label = $1)
               ORDER BY generated_at DESC
               LIMIT $2 OFFSET $3"#,
            scope_label,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(rows)
    }

    pub async fn get_package(&self, id: Uuid) -> Result<Option<EvidencePackageRecord>, DatabaseError> {
        let row = sqlx::query_as!(
            EvidencePackageRecord,
            "SELECT * FROM regulatory_evidence_packages WHERE id = $1",
            id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(row)
    }

    // ── Policy snapshots ──────────────────────────────────────────────────────

    pub async fn insert_policy_snapshot(
        &self,
        req: &CreatePolicySnapshotRequest,
    ) -> Result<PolicySnapshot, DatabaseError> {
        // Close the previous effective_until if open
        sqlx::query!(
            r#"UPDATE regulatory_policy_history
               SET effective_until = $1
               WHERE policy_name = $2 AND effective_until IS NULL"#,
            req.effective_from,
            req.policy_name,
        )
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        let row = sqlx::query_as!(
            PolicySnapshot,
            r#"INSERT INTO regulatory_policy_history
               (policy_name, policy_version, effective_from, effective_until,
                policy_state, changed_by, change_reason)
               VALUES ($1,$2,$3,$4,$5,$6,$7)
               RETURNING *"#,
            req.policy_name,
            req.policy_version,
            req.effective_from,
            req.effective_until,
            req.policy_state,
            req.changed_by,
            req.change_reason,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(row)
    }

    /// Return the policy state that was active at `at_time`.
    pub async fn policy_at(
        &self,
        policy_name: &str,
        at_time: DateTime<Utc>,
    ) -> Result<Option<PolicySnapshot>, DatabaseError> {
        let row = sqlx::query_as!(
            PolicySnapshot,
            r#"SELECT * FROM regulatory_policy_history
               WHERE policy_name = $1
                 AND effective_from <= $2
                 AND (effective_until IS NULL OR effective_until > $2)
               ORDER BY effective_from DESC LIMIT 1"#,
            policy_name,
            at_time,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(row)
    }

    pub async fn list_policy_history(
        &self,
        policy_name: &str,
    ) -> Result<Vec<PolicySnapshot>, DatabaseError> {
        let rows = sqlx::query_as!(
            PolicySnapshot,
            "SELECT * FROM regulatory_policy_history WHERE policy_name = $1 ORDER BY effective_from DESC",
            policy_name,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(rows)
    }

    pub async fn list_all_policy_names(&self) -> Result<Vec<String>, DatabaseError> {
        let rows = sqlx::query_scalar!(
            "SELECT DISTINCT policy_name FROM regulatory_policy_history ORDER BY policy_name"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(rows)
    }

    // ── System test reports ───────────────────────────────────────────────────

    pub async fn insert_test_report(
        &self,
        req: &CreateSystemTestReportRequest,
    ) -> Result<SystemTestReport, DatabaseError> {
        let row = sqlx::query_as!(
            SystemTestReport,
            r#"INSERT INTO regulatory_system_test_reports
               (report_type, report_label, executed_at, executed_by, outcome, summary, findings)
               VALUES ($1,$2,$3,$4,$5,$6,$7)
               RETURNING *"#,
            req.report_type,
            req.report_label,
            req.executed_at,
            req.executed_by,
            req.outcome,
            req.summary,
            req.findings,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(row)
    }

    pub async fn list_test_reports(
        &self,
        report_type: Option<&str>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<SystemTestReport>, DatabaseError> {
        let rows = sqlx::query_as!(
            SystemTestReport,
            r#"SELECT * FROM regulatory_system_test_reports
               WHERE ($1::text IS NULL OR report_type = $1)
                 AND ($2::timestamptz IS NULL OR executed_at >= $2)
                 AND ($3::timestamptz IS NULL OR executed_at <= $3)
               ORDER BY executed_at DESC LIMIT $4"#,
            report_type,
            from,
            to,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(rows)
    }

    // ── Collector queries (read-only cross-module aggregation) ────────────────

    /// Count AML events (CTR filings, SAR filings, screening hits) in range.
    pub async fn count_aml_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<i64, DatabaseError> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM api_audit_logs
               WHERE event_category = 'financial_transaction'
                 AND (event_type LIKE 'aml.%' OR event_type LIKE 'ctr.%' OR event_type LIKE 'sar.%')
                 AND created_at BETWEEN $1 AND $2"#,
            from,
            to,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(count.unwrap_or(0))
    }

    /// Count Travel Rule exchanges in range.
    pub async fn count_travel_rule_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<i64, DatabaseError> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM travel_rule_transfers WHERE created_at BETWEEN $1 AND $2",
            from,
            to,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(count.unwrap_or(0))
    }

    /// Count KYC/identity verification events in range.
    pub async fn count_kyc_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<i64, DatabaseError> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM api_audit_logs
               WHERE event_type LIKE 'kyc.%'
                 AND created_at BETWEEN $1 AND $2"#,
            from,
            to,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(count.unwrap_or(0))
    }

    /// Count multisig governance events in range.
    pub async fn count_multisig_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<i64, DatabaseError> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM multisig_proposals WHERE created_at BETWEEN $1 AND $2",
            from,
            to,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(count.unwrap_or(0))
    }

    /// Count policy snapshots active during range.
    pub async fn count_policy_snapshots_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<i64, DatabaseError> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM regulatory_policy_history
               WHERE effective_from <= $2
                 AND (effective_until IS NULL OR effective_until >= $1)"#,
            from,
            to,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(count.unwrap_or(0))
    }

    /// Count system test reports in range.
    pub async fn count_test_reports_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<i64, DatabaseError> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM regulatory_system_test_reports WHERE executed_at BETWEEN $1 AND $2",
            from,
            to,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(count.unwrap_or(0))
    }
}
