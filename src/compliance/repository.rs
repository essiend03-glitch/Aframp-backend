//! Database repository for the Compliance module — Issue #495.

use crate::compliance::models::*;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

pub struct ComplianceRepository {
    pool: PgPool,
}

impl ComplianceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Watchlist ─────────────────────────────────────────────────────────────

    pub async fn list_active_entries(&self) -> sqlx::Result<Vec<ComplianceWatchlistEntry>> {
        sqlx::query_as!(
            ComplianceWatchlistEntry,
            r#"SELECT id, full_name, aliases, passport_numbers, wallet_addresses,
                      list_source, match_threshold, active, created_at, updated_at
               FROM compliance_watchlists WHERE active = TRUE ORDER BY full_name"#
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_active(&self) -> sqlx::Result<i64> {
        sqlx::query_scalar!("SELECT COUNT(*) FROM compliance_watchlists WHERE active = TRUE")
            .fetch_one(&self.pool)
            .await
            .map(|c| c.unwrap_or(0))
    }

    pub async fn upsert_entry(
        &self,
        full_name: &str,
        aliases: &[String],
        wallet_addresses: &[String],
        list_source: &str,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"INSERT INTO compliance_watchlists
               (full_name, aliases, wallet_addresses, list_source)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (full_name, list_source) DO UPDATE SET
                 aliases          = EXCLUDED.aliases,
                 wallet_addresses = EXCLUDED.wallet_addresses,
                 updated_at       = NOW()"#,
            full_name,
            aliases,
            wallet_addresses,
            list_source,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Sanctions matches ─────────────────────────────────────────────────────

    pub async fn record_match(
        &self,
        transaction_id: Uuid,
        watchlist_entry_id: Uuid,
        matched_field: &str,
        match_score: f64,
    ) -> sqlx::Result<SanctionsMatchRecord> {
        use sqlx::types::BigDecimal;
        use std::str::FromStr;
        let score = BigDecimal::from_str(&format!("{:.4}", match_score)).unwrap_or_default();

        sqlx::query_as!(
            SanctionsMatchRecord,
            r#"INSERT INTO sanctions_matches
               (transaction_id, watchlist_entry_id, matched_field, match_score)
               VALUES ($1, $2, $3, $4)
               RETURNING *"#,
            transaction_id,
            watchlist_entry_id,
            matched_field,
            score,
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_pending_reviews(&self) -> sqlx::Result<Vec<SanctionsMatchRecord>> {
        sqlx::query_as!(
            SanctionsMatchRecord,
            "SELECT * FROM sanctions_matches WHERE status = 'HELD_FOR_REVIEW' ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_review(
        &self,
        id: Uuid,
        req: &ReviewDecisionRequest,
    ) -> sqlx::Result<SanctionsMatchRecord> {
        // Generate compliance certificate hash for approved transactions
        let cert_hash = if req.decision == "CLEARED" {
            let payload = format!("{}:{}:{}", id, req.reviewer_id, chrono::Utc::now().timestamp());
            Some(hex::encode(Sha256::digest(payload.as_bytes())))
        } else {
            None
        };

        sqlx::query_as!(
            SanctionsMatchRecord,
            r#"UPDATE sanctions_matches SET
               status           = $2,
               reviewer_id      = $3,
               reviewer_notes   = $4,
               compliance_cert_hash = $5,
               updated_at       = NOW()
               WHERE id = $1
               RETURNING *"#,
            id,
            req.decision,
            req.reviewer_id,
            req.notes.as_deref(),
            cert_hash.as_deref(),
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn last_sync_at(&self) -> sqlx::Result<Option<chrono::DateTime<chrono::Utc>>> {
        sqlx::query_scalar!(
            "SELECT MAX(updated_at) FROM compliance_watchlists"
        )
        .fetch_one(&self.pool)
        .await
    }
}
