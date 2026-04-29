//! PEP repository — persistence for matches, EDD cases, and audit log

use super::models::{
    PepAuditAction, PepEddCase, PepEddStatus, PepInfluenceLevel, PepMatch, PepMatchStatus,
    PepRelationshipType, PepRiskTier,
};
use super::monitoring::ConsumerForRescreening;
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

pub struct PepRepository {
    pool: Arc<PgPool>,
}

impl PepRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Persist screening matches, auto-create EDD cases for High/Critical tiers,
    /// and write an audit entry. Returns the EDD case ID if one was created.
    pub async fn save_screening_result(
        &self,
        consumer_id: Uuid,
        matches: &[PepMatch],
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let mut edd_case_id: Option<Uuid> = None;

        for m in matches {
            sqlx::query!(
                r#"
                INSERT INTO pep_matches (
                    match_id, consumer_id, matched_name, matched_aliases,
                    match_score, influence_level, relationship_type,
                    jurisdiction, cpi_score, risk_score, risk_tier,
                    status, provider_entity_id, screened_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
                ON CONFLICT (match_id) DO NOTHING
                "#,
                m.match_id,
                consumer_id,
                m.matched_name,
                &m.matched_aliases,
                m.match_score as i16,
                m.influence_level.as_str(),
                m.relationship_type.as_str(),
                m.jurisdiction,
                m.cpi_score as i16,
                m.risk_score,
                m.risk_tier.as_str(),
                m.status.as_str(),
                m.provider_entity_id.as_deref(),
                m.screened_at,
            )
            .execute(&mut *tx)
            .await?;

            // Auto-create EDD case for High/Critical matches that need review
            if m.status == PepMatchStatus::PendingReview && m.risk_tier.requires_edd() {
                let case_id = Uuid::new_v4();
                let requires_signoff = m.risk_tier.requires_senior_signoff();
                sqlx::query!(
                    r#"
                    INSERT INTO pep_edd_cases (
                        case_id, consumer_id, match_id, risk_tier,
                        status, requires_senior_signoff, created_at, updated_at
                    ) VALUES ($1,$2,$3,$4,$5,$6,NOW(),NOW())
                    "#,
                    case_id,
                    consumer_id,
                    m.match_id,
                    m.risk_tier.as_str(),
                    PepEddStatus::Open.as_str(),
                    requires_signoff,
                )
                .execute(&mut *tx)
                .await?;
                edd_case_id = Some(case_id);
            }
        }

        // Update last_pep_screened_at on kyc_records
        sqlx::query!(
            "UPDATE kyc_records SET last_pep_screened_at = NOW() WHERE consumer_id = $1",
            consumer_id
        )
        .execute(&mut *tx)
        .await
        .unwrap_or_else(|e| {
            // Non-fatal — column may not exist in older environments
            error!(error = %e, "Failed to update last_pep_screened_at");
            sqlx::postgres::PgQueryResult::default()
        });

        // Audit entry
        self.append_audit_entry_tx(
            &mut tx,
            consumer_id,
            PepAuditAction::ScreeningPerformed,
            None,
            serde_json::json!({ "match_count": matches.len() }),
        )
        .await?;

        tx.commit().await?;
        Ok(edd_case_id)
    }

    pub async fn fetch_matches_for_consumer(
        &self,
        consumer_id: Uuid,
    ) -> Result<Vec<PepMatch>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT match_id, matched_name, matched_aliases, match_score,
                   influence_level, relationship_type, jurisdiction, cpi_score,
                   risk_score, risk_tier, status, provider_entity_id,
                   screened_at, reviewed_at, reviewed_by, review_notes
            FROM pep_matches
            WHERE consumer_id = $1
            ORDER BY screened_at DESC
            "#,
            consumer_id
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| PepMatch {
                match_id: r.match_id,
                consumer_id,
                matched_name: r.matched_name,
                matched_aliases: r.matched_aliases,
                match_score: r.match_score as u8,
                influence_level: parse_influence_level(&r.influence_level),
                relationship_type: parse_relationship_type(&r.relationship_type),
                jurisdiction: r.jurisdiction,
                cpi_score: r.cpi_score as u8,
                risk_score: r.risk_score,
                risk_tier: parse_risk_tier(&r.risk_tier),
                status: parse_match_status(&r.status),
                provider_entity_id: r.provider_entity_id,
                screened_at: r.screened_at,
                reviewed_at: r.reviewed_at,
                reviewed_by: r.reviewed_by,
                review_notes: r.review_notes,
            })
            .collect())
    }

    /// Fetch the consumer_id for a given match (used in review handler).
    pub async fn get_consumer_id_for_match(
        &self,
        match_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        sqlx::query_scalar!(
            "SELECT consumer_id FROM pep_matches WHERE match_id = $1",
            match_id
        )
        .fetch_optional(&*self.pool)
        .await
    }

    /// Fetch consumers due for re-screening (last screened > 24 h ago or never).
    pub async fn fetch_consumers_for_rescreening(
        &self,
    ) -> Result<Vec<ConsumerForRescreening>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT k.consumer_id,
                   COALESCE(k.full_name, '') AS "full_name!",
                   k.date_of_birth,
                   k.nationality,
                   k.country_of_residence
            FROM kyc_records k
            WHERE k.status = 'approved'
              AND (
                k.last_pep_screened_at IS NULL
                OR k.last_pep_screened_at < NOW() - INTERVAL '24 hours'
              )
            LIMIT 5000
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ConsumerForRescreening {
                consumer_id: r.consumer_id,
                full_name: r.full_name,
                date_of_birth: r.date_of_birth,
                nationality: r.nationality,
                country_of_residence: r.country_of_residence,
            })
            .collect())
    }

    pub async fn update_match_status(
        &self,
        match_id: Uuid,
        status: PepMatchStatus,
        reviewer_id: Uuid,
        notes: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE pep_matches
            SET status = $1, reviewed_at = NOW(), reviewed_by = $2, review_notes = $3
            WHERE match_id = $4
            "#,
            status.as_str(),
            reviewer_id,
            notes,
            match_id,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_edd_case(
        &self,
        case_id: Uuid,
        status: PepEddStatus,
        actor_id: Uuid,
        sow_notes: Option<&str>,
        sof_notes: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE pep_edd_cases
            SET status = $1, updated_at = NOW(),
                source_of_wealth_notes = COALESCE($2, source_of_wealth_notes),
                source_of_funds_notes  = COALESCE($3, source_of_funds_notes),
                senior_signoff_by = CASE WHEN $1 = 'approved' THEN $4 ELSE senior_signoff_by END,
                senior_signoff_at = CASE WHEN $1 = 'approved' THEN NOW() ELSE senior_signoff_at END
            WHERE case_id = $5
            "#,
            status.as_str(),
            sow_notes,
            sof_notes,
            actor_id,
            case_id,
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_edd_case(
        &self,
        case_id: Uuid,
    ) -> Result<Option<PepEddCase>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT case_id, consumer_id, match_id, risk_tier, status,
                   source_of_wealth_notes, source_of_funds_notes, assigned_to,
                   requires_senior_signoff, senior_signoff_by, senior_signoff_at,
                   created_at, updated_at
            FROM pep_edd_cases WHERE case_id = $1
            "#,
            case_id
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| PepEddCase {
            case_id: r.case_id,
            consumer_id: r.consumer_id,
            match_id: r.match_id,
            risk_tier: parse_risk_tier(&r.risk_tier),
            status: parse_edd_status(&r.status),
            source_of_wealth_notes: r.source_of_wealth_notes,
            source_of_funds_notes: r.source_of_funds_notes,
            assigned_to: r.assigned_to,
            requires_senior_signoff: r.requires_senior_signoff,
            senior_signoff_by: r.senior_signoff_by,
            senior_signoff_at: r.senior_signoff_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    pub async fn list_open_edd_cases(&self) -> Result<Vec<PepEddCase>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT case_id, consumer_id, match_id, risk_tier, status,
                   source_of_wealth_notes, source_of_funds_notes, assigned_to,
                   requires_senior_signoff, senior_signoff_by, senior_signoff_at,
                   created_at, updated_at
            FROM pep_edd_cases
            WHERE status NOT IN ('approved', 'rejected')
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| PepEddCase {
                case_id: r.case_id,
                consumer_id: r.consumer_id,
                match_id: r.match_id,
                risk_tier: parse_risk_tier(&r.risk_tier),
                status: parse_edd_status(&r.status),
                source_of_wealth_notes: r.source_of_wealth_notes,
                source_of_funds_notes: r.source_of_funds_notes,
                assigned_to: r.assigned_to,
                requires_senior_signoff: r.requires_senior_signoff,
                senior_signoff_by: r.senior_signoff_by,
                senior_signoff_at: r.senior_signoff_at,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    pub async fn get_audit_log(
        &self,
        consumer_id: Uuid,
    ) -> Result<Vec<super::models::PepAuditEntry>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT entry_id, consumer_id, action, actor_id, details, chain_hash, created_at
            FROM pep_audit_log
            WHERE consumer_id = $1
            ORDER BY created_at ASC
            "#,
            consumer_id
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| super::models::PepAuditEntry {
                entry_id: r.entry_id,
                consumer_id: r.consumer_id,
                action: parse_audit_action(&r.action),
                actor_id: r.actor_id,
                details: r.details,
                created_at: r.created_at,
                chain_hash: r.chain_hash,
            })
            .collect())
    }

    pub async fn append_audit_entry(
        &self,
        consumer_id: Uuid,
        action: PepAuditAction,
        actor_id: Option<Uuid>,
        details: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        self.append_audit_entry_tx(&mut tx, consumer_id, action, actor_id, details)
            .await?;
        tx.commit().await
    }

    async fn append_audit_entry_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        consumer_id: Uuid,
        action: PepAuditAction,
        actor_id: Option<Uuid>,
        details: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        let last_hash: Option<String> = sqlx::query_scalar!(
            "SELECT chain_hash FROM pep_audit_log WHERE consumer_id = $1 ORDER BY created_at DESC LIMIT 1",
            consumer_id
        )
        .fetch_optional(&mut **tx)
        .await?;

        let entry_id = Uuid::new_v4();
        let action_str = action.to_string();
        let content = format!(
            "{}{}{}{}",
            entry_id,
            consumer_id,
            action_str,
            details.to_string()
        );
        let chain_hash = sha256_hex(&format!(
            "{}{}",
            last_hash.as_deref().unwrap_or(""),
            content
        ));

        sqlx::query!(
            r#"
            INSERT INTO pep_audit_log (entry_id, consumer_id, action, actor_id, details, chain_hash, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
            entry_id,
            consumer_id,
            action_str,
            actor_id,
            details,
            chain_hash,
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

// ---------------------------------------------------------------------------
// DB string → enum parsers
// ---------------------------------------------------------------------------

fn parse_influence_level(s: &str) -> PepInfluenceLevel {
    match s {
        "HEAD_OF_STATE" => PepInfluenceLevel::HeadOfState,
        "NATIONAL_SENIOR" => PepInfluenceLevel::NationalSenior,
        "REGIONAL_SENIOR" => PepInfluenceLevel::RegionalSenior,
        "STATE_ENTERPRISE_EXEC" => PepInfluenceLevel::StateEnterpriseExec,
        _ => PepInfluenceLevel::LocalOfficial,
    }
}

fn parse_relationship_type(s: &str) -> PepRelationshipType {
    match s {
        "IMMEDIATE_FAMILY" => PepRelationshipType::ImmediateFamily,
        "CLOSE_ASSOCIATE" => PepRelationshipType::CloseAssociate,
        _ => PepRelationshipType::DirectPep,
    }
}

fn parse_risk_tier(s: &str) -> PepRiskTier {
    match s {
        "CRITICAL" => PepRiskTier::Critical,
        "HIGH" => PepRiskTier::High,
        "MEDIUM" => PepRiskTier::Medium,
        _ => PepRiskTier::Low,
    }
}

fn parse_match_status(s: &str) -> PepMatchStatus {
    match s {
        "confirmed" => PepMatchStatus::Confirmed,
        "false_positive" => PepMatchStatus::FalsePositive,
        "auto_suppressed" => PepMatchStatus::AutoSuppressed,
        _ => PepMatchStatus::PendingReview,
    }
}

fn parse_edd_status(s: &str) -> PepEddStatus {
    match s {
        "in_progress" => PepEddStatus::InProgress,
        "pending_signoff" => PepEddStatus::PendingSignoff,
        "approved" => PepEddStatus::Approved,
        "rejected" => PepEddStatus::Rejected,
        _ => PepEddStatus::Open,
    }
}

fn parse_audit_action(s: &str) -> PepAuditAction {
    match s {
        "MATCH_CONFIRMED" => PepAuditAction::MatchConfirmed,
        "MATCH_DISMISSED" => PepAuditAction::MatchDismissed,
        "AUTO_SUPPRESSED" => PepAuditAction::AutoSuppressed,
        "EDD_CASE_CREATED" => PepAuditAction::EddCaseCreated,
        "EDD_CASE_UPDATED" => PepAuditAction::EddCaseUpdated,
        "SENIOR_SIGNOFF_GRANTED" => PepAuditAction::SeniorSignoffGranted,
        "SENIOR_SIGNOFF_DENIED" => PepAuditAction::SeniorSignoffDenied,
        "RESCREENING_SCHEDULED" => PepAuditAction::RescreeningScheduled,
        "STATUS_CHANGED" => PepAuditAction::StatusChanged,
        _ => PepAuditAction::ScreeningPerformed,
    }
}

// ---------------------------------------------------------------------------
// as_str helpers for DB serialisation
// ---------------------------------------------------------------------------

impl PepMatchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PepMatchStatus::PendingReview => "pending_review",
            PepMatchStatus::Confirmed => "confirmed",
            PepMatchStatus::FalsePositive => "false_positive",
            PepMatchStatus::AutoSuppressed => "auto_suppressed",
        }
    }
}

impl PepEddStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PepEddStatus::Open => "open",
            PepEddStatus::InProgress => "in_progress",
            PepEddStatus::PendingSignoff => "pending_signoff",
            PepEddStatus::Approved => "approved",
            PepEddStatus::Rejected => "rejected",
        }
    }
}
