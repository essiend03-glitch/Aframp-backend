use crate::sla::models::*;
use chrono::{Datelike, NaiveDate, Utc};
use sqlx::types::BigDecimal;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

pub struct SlaRepository {
    pool: PgPool,
}

impl SlaRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── SLO definitions ───────────────────────────────────────────────────────

    pub async fn list_slos(&self) -> sqlx::Result<Vec<SloDefinition>> {
        sqlx::query_as!(
            SloDefinition,
            "SELECT * FROM slo_definitions WHERE enabled = TRUE ORDER BY severity, name"
        )
        .fetch_all(&self.pool)
        .await
    }

    // ── Breach incidents ──────────────────────────────────────────────────────

    pub async fn open_incident(
        &self,
        slo_id: Uuid,
        observed_value: f64,
        threshold_value: f64,
        affected_service: &str,
        context: serde_json::Value,
    ) -> sqlx::Result<SlaBreachIncident> {
        let obs = BigDecimal::from_str(&observed_value.to_string()).unwrap_or_default();
        let thr = BigDecimal::from_str(&threshold_value.to_string()).unwrap_or_default();
        sqlx::query_as!(
            SlaBreachIncident,
            r#"INSERT INTO sla_breach_incidents
               (slo_id, observed_value, threshold_value, affected_service, context_snapshot)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING *"#,
            slo_id,
            obs,
            thr,
            affected_service,
            context,
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_incident(
        &self,
        id: Uuid,
        req: &UpdateIncidentRequest,
    ) -> sqlx::Result<SlaBreachIncident> {
        sqlx::query_as!(
            SlaBreachIncident,
            r#"UPDATE sla_breach_incidents SET
               status               = COALESCE($2, status),
               root_cause_summary   = COALESCE($3, root_cause_summary),
               remediation_steps    = COALESCE($4, remediation_steps),
               etr                  = COALESCE($5, etr),
               resolved_at          = CASE WHEN $2 = 'resolved' THEN NOW() ELSE resolved_at END,
               updated_at           = NOW()
               WHERE id = $1
               RETURNING *"#,
            id,
            req.status.as_deref(),
            req.root_cause_summary.as_deref(),
            req.remediation_steps.as_deref(),
            req.etr,
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn mark_partners_notified(&self, id: Uuid) -> sqlx::Result<()> {
        sqlx::query!(
            "UPDATE sla_breach_incidents SET partners_notified = TRUE, notification_sent_at = NOW(), updated_at = NOW() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_open_incidents(&self) -> sqlx::Result<Vec<SlaBreachIncident>> {
        sqlx::query_as!(
            SlaBreachIncident,
            "SELECT * FROM sla_breach_incidents WHERE status NOT IN ('closed') ORDER BY detected_at DESC"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_incidents_by_status(
        &self,
        status: Option<&str>,
    ) -> sqlx::Result<Vec<SlaBreachIncident>> {
        match status {
            Some(s) => {
                sqlx::query_as!(
                SlaBreachIncident,
                "SELECT * FROM sla_breach_incidents WHERE status = $1 ORDER BY detected_at DESC",
                s
            )
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as!(
                    SlaBreachIncident,
                    "SELECT * FROM sla_breach_incidents ORDER BY detected_at DESC LIMIT 100"
                )
                .fetch_all(&self.pool)
                .await
            }
        }
    }

    pub async fn get_incident(&self, id: Uuid) -> sqlx::Result<Option<SlaBreachIncident>> {
        sqlx::query_as!(
            SlaBreachIncident,
            "SELECT * FROM sla_breach_incidents WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    // ── Post-mortems ──────────────────────────────────────────────────────────

    pub async fn create_post_mortem(
        &self,
        incident_id: Uuid,
        req: &CreatePostMortemRequest,
    ) -> sqlx::Result<SlaPostMortem> {
        sqlx::query_as!(
            SlaPostMortem,
            r#"INSERT INTO sla_post_mortems
               (incident_id, author, timeline, root_cause, contributing_factors,
                remediation, preventive_measures, action_items)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               RETURNING *"#,
            incident_id,
            req.author,
            req.timeline,
            req.root_cause,
            req.contributing_factors.as_deref(),
            req.remediation,
            req.preventive_measures,
            req.action_items,
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_post_mortem(&self, incident_id: Uuid) -> sqlx::Result<Option<SlaPostMortem>> {
        sqlx::query_as!(
            SlaPostMortem,
            "SELECT * FROM sla_post_mortems WHERE incident_id = $1",
            incident_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    // ── Compliance reports ────────────────────────────────────────────────────

    pub async fn generate_monthly_report(
        &self,
        partner_id: Option<Uuid>,
        month: NaiveDate,
    ) -> sqlx::Result<SlaComplianceReport> {
        let month_start = NaiveDate::from_ymd_opt(month.year(), month.month(), 1).unwrap_or(month);
        let month_end = {
            let (y, m) = if month.month() == 12 {
                (month.year() + 1, 1)
            } else {
                (month.year(), month.month() + 1)
            };
            NaiveDate::from_ymd_opt(y, m, 1).unwrap()
        };

        // Collect breach IDs for the month
        let breach_ids: Vec<Uuid> = sqlx::query_scalar!(
            "SELECT id FROM sla_breach_incidents WHERE detected_at >= $1 AND detected_at < $2",
            month_start.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            month_end.and_hms_opt(0, 0, 0).unwrap().and_utc(),
        )
        .fetch_all(&self.pool)
        .await?;

        let total = breach_ids.len() as i32;

        // MTTR: average seconds between detected_at and resolved_at
        let mttr: Option<BigDecimal> = sqlx::query_scalar!(
            r#"SELECT AVG(EXTRACT(EPOCH FROM (resolved_at - detected_at)))
               FROM sla_breach_incidents
               WHERE detected_at >= $1 AND detected_at < $2 AND resolved_at IS NOT NULL"#,
            month_start.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            month_end.and_hms_opt(0, 0, 0).unwrap().and_utc(),
        )
        .fetch_one(&self.pool)
        .await?;

        // Availability: 100 - (total_breach_window_seconds / month_seconds * 100)
        let month_secs = (month_end - month_start).num_seconds() as f64;
        let breach_secs: Option<f64> = sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(EXTRACT(EPOCH FROM
                 COALESCE(resolved_at, NOW()) - detected_at)), 0)
               FROM sla_breach_incidents
               WHERE detected_at >= $1 AND detected_at < $2"#,
            month_start.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            month_end.and_hms_opt(0, 0, 0).unwrap().and_utc(),
        )
        .fetch_one(&self.pool)
        .await?;

        let availability = breach_secs.map(|bs| {
            let pct = 100.0 - (bs / month_secs * 100.0);
            BigDecimal::from_str(&format!("{:.4}", pct.max(0.0))).unwrap_or_default()
        });

        sqlx::query_as!(
            SlaComplianceReport,
            r#"INSERT INTO sla_compliance_reports
               (partner_id, report_month, total_breaches, mttr_seconds, availability_pct, breach_ids)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (partner_id, report_month) DO UPDATE SET
                 total_breaches   = EXCLUDED.total_breaches,
                 mttr_seconds     = EXCLUDED.mttr_seconds,
                 availability_pct = EXCLUDED.availability_pct,
                 breach_ids       = EXCLUDED.breach_ids,
                 generated_at     = NOW()
               RETURNING *"#,
            partner_id,
            month_start,
            total,
            mttr,
            availability,
            &breach_ids,
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_reports(
        &self,
        partner_id: Option<Uuid>,
    ) -> sqlx::Result<Vec<SlaComplianceReport>> {
        sqlx::query_as!(
            SlaComplianceReport,
            r#"SELECT * FROM sla_compliance_reports
               WHERE ($1::uuid IS NULL OR partner_id = $1)
               ORDER BY report_month DESC LIMIT 24"#,
            partner_id,
        )
        .fetch_all(&self.pool)
        .await
    }

    // ── Dashboard aggregates ──────────────────────────────────────────────────

    pub async fn dashboard_stats(&self) -> sqlx::Result<(i64, Option<f64>, Option<f64>)> {
        let since = Utc::now() - chrono::Duration::days(30);

        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM sla_breach_incidents WHERE detected_at >= $1",
            since
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        let mttr: Option<f64> = sqlx::query_scalar!(
            r#"SELECT AVG(EXTRACT(EPOCH FROM (resolved_at - detected_at)))
               FROM sla_breach_incidents
               WHERE detected_at >= $1 AND resolved_at IS NOT NULL"#,
            since
        )
        .fetch_one(&self.pool)
        .await?;

        let breach_secs: Option<f64> = sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(EXTRACT(EPOCH FROM
                 COALESCE(resolved_at, NOW()) - detected_at)), 0)
               FROM sla_breach_incidents WHERE detected_at >= $1"#,
            since
        )
        .fetch_one(&self.pool)
        .await?;

        let avail = breach_secs.map(|bs| {
            let month_secs = 30.0 * 86400.0;
            (100.0 - (bs / month_secs * 100.0)).max(0.0)
        });

        Ok((count, mttr, avail))
    }
}

    // ── Issue #464: SLA Policies ──────────────────────────────────────────────

    pub async fn list_active_policies(&self) -> sqlx::Result<Vec<crate::sla::models::SlaPolicy>> {
        sqlx::query_as!(
            crate::sla::models::SlaPolicy,
            "SELECT * FROM sla_policies WHERE enabled = TRUE ORDER BY corridor_id"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn upsert_policy(
        &self,
        req: &crate::sla::models::CreatePolicyRequest,
    ) -> sqlx::Result<crate::sla::models::SlaPolicy> {
        sqlx::query_as!(
            crate::sla::models::SlaPolicy,
            r#"INSERT INTO sla_policies (corridor_id, metric, threshold_ms)
               VALUES ($1, $2, $3)
               ON CONFLICT (corridor_id, metric) DO UPDATE SET
                 threshold_ms = EXCLUDED.threshold_ms,
                 updated_at   = NOW()
               RETURNING *"#,
            req.corridor_id,
            req.metric,
            req.threshold_ms,
        )
        .fetch_one(&self.pool)
        .await
    }

    // ── Issue #464: Breach Events ─────────────────────────────────────────────

    pub async fn insert_breach_event(
        &self,
        policy_id: uuid::Uuid,
        corridor_id: &str,
        observed_ms: f64,
        threshold_ms: f64,
    ) -> sqlx::Result<uuid::Uuid> {
        use sqlx::types::BigDecimal;
        use std::str::FromStr;
        let obs = BigDecimal::from_str(&observed_ms.to_string()).unwrap_or_default();
        let thr = BigDecimal::from_str(&threshold_ms.to_string()).unwrap_or_default();
        let row = sqlx::query!(
            r#"INSERT INTO sla_breach_events (policy_id, corridor_id, observed_ms, threshold_ms)
               VALUES ($1, $2, $3, $4) RETURNING id"#,
            policy_id,
            corridor_id,
            obs,
            thr,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.id)
    }

    pub async fn list_breach_events(
        &self,
        limit: i64,
    ) -> sqlx::Result<Vec<crate::sla::models::SlaBreachEvent>> {
        sqlx::query_as!(
            crate::sla::models::SlaBreachEvent,
            "SELECT * FROM sla_breach_events ORDER BY created_at DESC LIMIT $1",
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn resolve_breach_event(
        &self,
        id: uuid::Uuid,
        root_cause: &str,
        actor: &str,
    ) -> sqlx::Result<()> {
        // Breach events are immutable — create an audit override entry instead
        sqlx::query!(
            r#"INSERT INTO sla_breach_audit_overrides (breach_event_id, root_cause, actor)
               VALUES ($1, $2, $3)"#,
            id,
            root_cause,
            actor,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_partner_webhook_endpoints(
        &self,
        corridor_id: &str,
    ) -> sqlx::Result<Vec<String>> {
        let rows = sqlx::query!(
            "SELECT url FROM sla_partner_webhooks WHERE corridor_id = $1 AND enabled = TRUE",
            corridor_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.url).collect())
    }
