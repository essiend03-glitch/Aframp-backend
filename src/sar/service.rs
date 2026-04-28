//! SAR service — auto-draft generation and data aggregation
//!
//! Called by the AML case manager when a Critical or Medium flag is raised.
//! Aggregates the last 48 hours of account activity, renders the regulatory
//! template, and persists a Draft SAR within the 30-minute SLA.

use std::sync::Arc;

use chrono::Utc;
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

use super::{
    models::{
        ActivitySnapshot, RegulatoryAuthority, SarReport, SarStatus, TransactionSummary,
    },
    repository::SarRepository,
    template,
};

pub struct SarService {
    repo: SarRepository,
    pool: PgPool,
}

impl SarService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: SarRepository::new(pool.clone()),
            pool,
        }
    }

    /// Auto-generate a SAR draft from an AML case.
    /// Returns the persisted draft (or an existing one if already created for this case).
    pub async fn auto_draft(
        &self,
        aml_case_id: Uuid,
        transaction_id: Uuid,
        wallet_address: &str,
    ) -> Result<SarReport, anyhow::Error> {
        // Idempotency: return existing draft if already created
        if let Some(existing) = self.find_by_case(aml_case_id).await? {
            return Ok(existing);
        }

        let snapshot = self.aggregate_activity(wallet_address, 48).await?;
        let authority = RegulatoryAuthority::Nfiu; // default; can be overridden per corridor
        let rendered = template::render(
            &SarReport {
                id: Uuid::nil(), // placeholder for rendering
                aml_case_id,
                transaction_id,
                wallet_address: wallet_address.to_owned(),
                status: SarStatus::Draft.to_string(),
                authority: authority.to_string(),
                activity_snapshot: serde_json::to_value(&snapshot)?,
                rendered_report: None,
                reviewed_by: None,
                review_notes: None,
                filed_at: None,
                acknowledged_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            &snapshot,
            &authority,
        );

        let now = Utc::now();
        let report = SarReport {
            id: Uuid::new_v4(),
            aml_case_id,
            transaction_id,
            wallet_address: wallet_address.to_owned(),
            status: SarStatus::Draft.to_string(),
            authority: authority.to_string(),
            activity_snapshot: serde_json::to_value(&snapshot)?,
            rendered_report: Some(rendered),
            reviewed_by: None,
            review_notes: None,
            filed_at: None,
            acknowledged_at: None,
            created_at: now,
            updated_at: now,
        };

        let saved = self.repo.create(&report).await?;

        // Append initial audit entry
        sqlx::query!(
            r#"
            INSERT INTO sar_audit_log (id, sar_id, actor_id, action, from_status, to_status, created_at)
            VALUES ($1,$2,'system','auto_draft','','Draft',NOW())
            "#,
            Uuid::new_v4(),
            saved.id,
        )
        .execute(&self.pool)
        .await?;

        info!(
            sar_id = %saved.id,
            aml_case_id = %aml_case_id,
            wallet = %wallet_address,
            "SAR draft auto-generated"
        );

        Ok(saved)
    }

    /// Submit draft to the review queue (Draft → PendingReview).
    pub async fn submit_for_review(&self, sar_id: Uuid) -> Result<SarReport, anyhow::Error> {
        self.repo
            .transition(sar_id, "PendingReview", "system", Some("Auto-submitted for review"), None)
            .await
    }

    /// Compliance officer approves the SAR (PendingReview → Approved).
    pub async fn approve(
        &self,
        sar_id: Uuid,
        officer_id: &str,
        notes: Option<&str>,
        amended_report: Option<&str>,
    ) -> Result<SarReport, anyhow::Error> {
        self.repo
            .transition(sar_id, "Approved", officer_id, notes, amended_report)
            .await
    }

    /// Compliance officer rejects the SAR (PendingReview → Rejected).
    pub async fn reject(
        &self,
        sar_id: Uuid,
        officer_id: &str,
        notes: Option<&str>,
    ) -> Result<SarReport, anyhow::Error> {
        self.repo
            .transition(sar_id, "Rejected", officer_id, notes, None)
            .await
    }

    /// Mark as filed after transmission to regulator (Approved → Filed).
    pub async fn mark_filed(&self, sar_id: Uuid) -> Result<SarReport, anyhow::Error> {
        self.repo
            .transition(sar_id, "Filed", "system", Some("Transmitted to regulator"), None)
            .await
    }

    /// Regulator acknowledged receipt (Filed → Acknowledged).
    pub async fn acknowledge(&self, sar_id: Uuid, ref_number: &str) -> Result<SarReport, anyhow::Error> {
        self.repo
            .transition(
                sar_id,
                "Acknowledged",
                "system",
                Some(&format!("Regulator ref: {ref_number}")),
                None,
            )
            .await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<SarReport>, anyhow::Error> {
        self.repo.get(id).await
    }

    pub async fn list_pending(&self) -> Result<Vec<SarReport>, anyhow::Error> {
        self.repo.list_by_status("PendingReview").await
    }

    pub async fn get_audit_log(&self, sar_id: Uuid) -> Result<Vec<super::models::SarAuditEntry>, anyhow::Error> {
        self.repo.get_audit_log(sar_id).await
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    async fn find_by_case(&self, aml_case_id: Uuid) -> Result<Option<SarReport>, anyhow::Error> {
        Ok(sqlx::query_as!(
            SarReport,
            "SELECT * FROM sar_reports WHERE aml_case_id = $1 LIMIT 1",
            aml_case_id
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Aggregate the last `window_hours` of activity for a wallet.
    async fn aggregate_activity(
        &self,
        wallet_address: &str,
        window_hours: u32,
    ) -> Result<ActivitySnapshot, anyhow::Error> {
        let since = Utc::now() - chrono::Duration::hours(window_hours as i64);

        // Transaction count + volume
        let agg: Option<(i64, Option<String>)> = sqlx::query_as(
            r#"
            SELECT COUNT(*), SUM(from_amount)::TEXT
            FROM transactions
            WHERE wallet_address = $1 AND created_at >= $2
            "#,
        )
        .bind(wallet_address)
        .bind(since)
        .fetch_optional(&self.pool)
        .await?;

        let (tx_count, total_volume) = agg.unwrap_or((0, None));

        // Recent transactions (last 20)
        let rows: Vec<(Uuid, String, String, String, String, chrono::DateTime<Utc>)> =
            sqlx::query_as(
                r#"
                SELECT transaction_id, type, from_amount::TEXT, from_currency, status, created_at
                FROM transactions
                WHERE wallet_address = $1 AND created_at >= $2
                ORDER BY created_at DESC LIMIT 20
                "#,
            )
            .bind(wallet_address)
            .bind(since)
            .fetch_all(&self.pool)
            .await?;

        let recent_transactions = rows
            .into_iter()
            .map(|(id, tx_type, amount, currency, status, created_at)| TransactionSummary {
                transaction_id: id,
                tx_type,
                amount,
                currency,
                status,
                created_at,
            })
            .collect();

        // IP addresses from audit log (best-effort)
        let ip_rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT ip_address FROM audit_log
            WHERE actor_id = $1 AND created_at >= $2 AND ip_address IS NOT NULL
            LIMIT 20
            "#,
        )
        .bind(wallet_address)
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let ip_addresses = ip_rows.into_iter().map(|(ip,)| ip).collect();

        // Linked bank accounts from KYC
        let bank_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT account_number FROM bank_accounts WHERE wallet_address = $1",
        )
        .bind(wallet_address)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let linked_bank_accounts = bank_rows.into_iter().map(|(a,)| a).collect();

        Ok(ActivitySnapshot {
            wallet_address: wallet_address.to_owned(),
            window_hours,
            transaction_count: tx_count,
            total_volume: total_volume.unwrap_or_else(|| "0".into()),
            ip_addresses,
            linked_bank_accounts,
            recent_transactions,
            captured_at: Utc::now(),
        })
    }
}
