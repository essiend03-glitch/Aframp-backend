//! Database access layer for merchant CRM.

use crate::database::error::DatabaseError;
use crate::merchant_crm::models::*;
use sqlx::PgPool;
use uuid::Uuid;

pub struct CustomerProfileRepository {
    pool: PgPool,
}

impl CustomerProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -------------------------------------------------------------------------
    // Customer Profiles
    // -------------------------------------------------------------------------

    /// Upsert a customer profile (insert or update on conflict).
    pub async fn upsert_profile(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
        display_name: Option<&str>,
        encrypted_email: Option<&str>,
        encrypted_phone: Option<&str>,
        encrypted_name: Option<&str>,
        consent_given: bool,
        consent_ip: Option<&str>,
        tags: &[String],
    ) -> Result<CustomerProfile, DatabaseError> {
        let tags_arr: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();
        sqlx::query_as::<_, CustomerProfile>(
            r#"
            INSERT INTO merchant_customer_profiles
                (merchant_id, wallet_address, display_name,
                 encrypted_email, encrypted_phone, encrypted_name,
                 consent_given, consent_given_at, consent_ip_address, tags)
            VALUES ($1, $2, $3, $4, $5, $6, $7,
                    CASE WHEN $7 THEN now() ELSE NULL END,
                    $8::inet, $9)
            ON CONFLICT (merchant_id, wallet_address) DO UPDATE SET
                display_name      = COALESCE(EXCLUDED.display_name, merchant_customer_profiles.display_name),
                encrypted_email   = COALESCE(EXCLUDED.encrypted_email, merchant_customer_profiles.encrypted_email),
                encrypted_phone   = COALESCE(EXCLUDED.encrypted_phone, merchant_customer_profiles.encrypted_phone),
                encrypted_name    = COALESCE(EXCLUDED.encrypted_name, merchant_customer_profiles.encrypted_name),
                consent_given     = EXCLUDED.consent_given,
                consent_given_at  = CASE WHEN EXCLUDED.consent_given THEN now()
                                         ELSE merchant_customer_profiles.consent_given_at END,
                consent_ip_address = COALESCE(EXCLUDED.consent_ip_address, merchant_customer_profiles.consent_ip_address),
                tags              = EXCLUDED.tags,
                updated_at        = now()
            RETURNING *
            "#,
        )
        .bind(merchant_id)
        .bind(wallet_address)
        .bind(display_name)
        .bind(encrypted_email)
        .bind(encrypted_phone)
        .bind(encrypted_name)
        .bind(consent_given)
        .bind(consent_ip)
        .bind(tags_arr)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Refresh lifetime metrics from the transactions table.
    pub async fn refresh_profile_metrics(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE merchant_customer_profiles p
            SET
                total_spent          = COALESCE(agg.total_spent, 0),
                total_transactions   = COALESCE(agg.cnt, 0),
                first_transaction_at = agg.first_tx,
                last_transaction_at  = agg.last_tx,
                is_repeat_customer   = COALESCE(agg.cnt, 0) > 1,
                updated_at           = now()
            FROM (
                SELECT
                    SUM(cngn_amount)  AS total_spent,
                    COUNT(*)          AS cnt,
                    MIN(created_at)   AS first_tx,
                    MAX(created_at)   AS last_tx
                FROM transactions
                WHERE wallet_address = $2
                  AND status = 'completed'
            ) agg
            WHERE p.merchant_id = $1
              AND p.wallet_address = $2
            "#,
        )
        .bind(merchant_id)
        .bind(wallet_address)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    /// Find a single profile.
    pub async fn find_profile(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
    ) -> Result<Option<CustomerProfile>, DatabaseError> {
        sqlx::query_as::<_, CustomerProfile>(
            "SELECT * FROM merchant_customer_profiles WHERE merchant_id = $1 AND wallet_address = $2",
        )
        .bind(merchant_id)
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// List profiles with optional filters (pagination).
    pub async fn list_profiles(
        &self,
        merchant_id: Uuid,
        min_spent: Option<f64>,
        active_within_days: Option<i32>,
        tag: Option<&str>,
        repeat_only: bool,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<CustomerProfile>, DatabaseError> {
        let offset = (page - 1) * page_size;
        sqlx::query_as::<_, CustomerProfile>(
            r#"
            SELECT * FROM merchant_customer_profiles
            WHERE merchant_id = $1
              AND ($2::numeric IS NULL OR total_spent >= $2)
              AND ($3::int IS NULL OR last_transaction_at >= now() - ($3 || ' days')::interval)
              AND ($4::text IS NULL OR $4 = ANY(tags))
              AND (NOT $5 OR is_repeat_customer = TRUE)
            ORDER BY total_spent DESC
            LIMIT $6 OFFSET $7
            "#,
        )
        .bind(merchant_id)
        .bind(min_spent)
        .bind(active_within_days)
        .bind(tag)
        .bind(repeat_only)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Add or remove a tag on a profile.
    pub async fn update_tags(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
        tags: &[String],
    ) -> Result<(), DatabaseError> {
        let tags_arr: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();
        sqlx::query(
            "UPDATE merchant_customer_profiles SET tags = $3, updated_at = now()
             WHERE merchant_id = $1 AND wallet_address = $2",
        )
        .bind(merchant_id)
        .bind(wallet_address)
        .bind(tags_arr)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Retention Metrics
    // -------------------------------------------------------------------------

    pub async fn get_retention_metrics(
        &self,
        merchant_id: Uuid,
    ) -> Result<(i64, i64), DatabaseError> {
        let row: (i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) AS total,
                COUNT(*) FILTER (WHERE is_repeat_customer) AS repeat_count
            FROM merchant_customer_profiles
            WHERE merchant_id = $1
            "#,
        )
        .bind(merchant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(row)
    }

    // -------------------------------------------------------------------------
    // Segments
    // -------------------------------------------------------------------------

    pub async fn upsert_segment(
        &self,
        merchant_id: Uuid,
        name: &str,
        description: Option<&str>,
        filter_criteria: &serde_json::Value,
    ) -> Result<CustomerSegment, DatabaseError> {
        sqlx::query_as::<_, CustomerSegment>(
            r#"
            INSERT INTO merchant_customer_segments (merchant_id, name, description, filter_criteria)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (merchant_id, name) DO UPDATE SET
                description      = EXCLUDED.description,
                filter_criteria  = EXCLUDED.filter_criteria,
                updated_at       = now()
            RETURNING *
            "#,
        )
        .bind(merchant_id)
        .bind(name)
        .bind(description)
        .bind(filter_criteria)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_segments(
        &self,
        merchant_id: Uuid,
    ) -> Result<Vec<CustomerSegment>, DatabaseError> {
        sqlx::query_as::<_, CustomerSegment>(
            "SELECT * FROM merchant_customer_segments WHERE merchant_id = $1 ORDER BY name",
        )
        .bind(merchant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // -------------------------------------------------------------------------
    // Analytics Snapshots
    // -------------------------------------------------------------------------

    pub async fn upsert_analytics_snapshot(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
        avg_freq_days: Option<f64>,
        days_since_last: Option<i32>,
        avg_value: Option<f64>,
        max_value: Option<f64>,
        min_value: Option<f64>,
        retention_score: Option<f64>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO merchant_customer_analytics
                (merchant_id, wallet_address, avg_purchase_frequency_days,
                 days_since_last_purchase, avg_transaction_value,
                 max_transaction_value, min_transaction_value, retention_score)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (merchant_id, wallet_address, snapshot_date) DO UPDATE SET
                avg_purchase_frequency_days = EXCLUDED.avg_purchase_frequency_days,
                days_since_last_purchase    = EXCLUDED.days_since_last_purchase,
                avg_transaction_value       = EXCLUDED.avg_transaction_value,
                max_transaction_value       = EXCLUDED.max_transaction_value,
                min_transaction_value       = EXCLUDED.min_transaction_value,
                retention_score             = EXCLUDED.retention_score
            "#,
        )
        .bind(merchant_id)
        .bind(wallet_address)
        .bind(avg_freq_days)
        .bind(days_since_last)
        .bind(avg_value)
        .bind(max_value)
        .bind(min_value)
        .bind(retention_score)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }
}
