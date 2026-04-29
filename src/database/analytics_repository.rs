//! Database queries for wallet analytics (Issue #369).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::database::error::DatabaseError;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow)]
pub struct SnapshotRow {
    pub id: Uuid,
    pub wallet_address: String,
    pub period: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_tx_count: i32,
    pub total_cngn_sent: sqlx::types::BigDecimal,
    pub total_cngn_received: sqlx::types::BigDecimal,
    pub total_fiat_onramped: sqlx::types::BigDecimal,
    pub total_fiat_offramped: sqlx::types::BigDecimal,
    pub total_fees_paid: sqlx::types::BigDecimal,
    pub unique_counterparties: i32,
    pub most_used_tx_type: Option<String>,
    pub most_used_provider: Option<String>,
    pub active_days: i32,
    pub snapshot_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SpendingCategoryRow {
    pub id: Uuid,
    pub wallet_address: String,
    pub period: String,
    pub period_start: DateTime<Utc>,
    pub category: String,
    pub tx_count: i32,
    pub total_amount: sqlx::types::BigDecimal,
    pub percentage: sqlx::types::BigDecimal,
}

#[derive(Debug, Clone, FromRow)]
pub struct CounterpartyRow {
    pub id: Uuid,
    pub wallet_address: String,
    pub counterparty_id: String,
    pub counterparty_type: String,
    pub tx_count: i32,
    pub total_amount_sent: sqlx::types::BigDecimal,
    pub first_tx_at: DateTime<Utc>,
    pub last_tx_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct BehaviourProfileRow {
    pub id: Uuid,
    pub wallet_address: String,
    pub avg_tx_size: sqlx::types::BigDecimal,
    pub tx_frequency_per_week: sqlx::types::BigDecimal,
    pub preferred_hour_utc: Option<i16>,
    pub preferred_provider: Option<String>,
    pub preferred_currency_pair: Option<String>,
    pub risk_score: sqlx::types::BigDecimal,
    pub profile_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct InsightRow {
    pub id: Uuid,
    pub wallet_address: String,
    pub period: String,
    pub period_start: DateTime<Utc>,
    pub top_category: Option<String>,
    pub top_category_amount: Option<sqlx::types::BigDecimal>,
    pub prev_period_delta_pct: Option<sqlx::types::BigDecimal>,
    pub largest_tx_amount: Option<sqlx::types::BigDecimal>,
    pub largest_tx_id: Option<Uuid>,
    pub most_frequent_counterparty: Option<String>,
    pub estimated_monthly_fees: Option<sqlx::types::BigDecimal>,
    pub cngn_balance_trend: Option<String>,
    pub generated_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct AnomalyFlagRow {
    pub id: Uuid,
    pub wallet_address: String,
    pub anomaly_type: String,
    pub deviation_magnitude: sqlx::types::BigDecimal,
    pub flagged_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub routed_to_compliance: bool,
    pub compliance_case_id: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct AdminDailyAggRow {
    pub id: Uuid,
    pub agg_date: chrono::NaiveDate,
    pub total_wallets: i64,
    pub active_wallets: i64,
    pub new_wallets: i64,
    pub total_cngn_transferred: sqlx::types::BigDecimal,
    pub total_fiat_onramped: sqlx::types::BigDecimal,
    pub total_fiat_offramped: sqlx::types::BigDecimal,
    pub avg_tx_size: sqlx::types::BigDecimal,
    pub total_tx_count: i64,
    pub computed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

pub struct UpsertSnapshot {
    pub wallet_address: String,
    pub period: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_tx_count: i32,
    pub total_cngn_sent: sqlx::types::BigDecimal,
    pub total_cngn_received: sqlx::types::BigDecimal,
    pub total_fiat_onramped: sqlx::types::BigDecimal,
    pub total_fiat_offramped: sqlx::types::BigDecimal,
    pub total_fees_paid: sqlx::types::BigDecimal,
    pub unique_counterparties: i32,
    pub most_used_tx_type: Option<String>,
    pub most_used_provider: Option<String>,
    pub active_days: i32,
}

pub struct UpsertProfile {
    pub wallet_address: String,
    pub avg_tx_size: sqlx::types::BigDecimal,
    pub tx_frequency_per_week: sqlx::types::BigDecimal,
    pub preferred_hour_utc: Option<i16>,
    pub preferred_provider: Option<String>,
    pub preferred_currency_pair: Option<String>,
    pub risk_score: sqlx::types::BigDecimal,
}

// ---------------------------------------------------------------------------
// Repository
// ---------------------------------------------------------------------------

pub struct AnalyticsRepository {
    pool: PgPool,
}

impl AnalyticsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // --- Snapshots ---

    pub async fn upsert_snapshot(&self, s: UpsertSnapshot) -> Result<SnapshotRow, DatabaseError> {
        sqlx::query_as::<_, SnapshotRow>(
            r#"INSERT INTO wallet_usage_snapshots
               (wallet_address, period, period_start, period_end,
                total_tx_count, total_cngn_sent, total_cngn_received,
                total_fiat_onramped, total_fiat_offramped, total_fees_paid,
                unique_counterparties, most_used_tx_type, most_used_provider, active_days)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
               ON CONFLICT (wallet_address, period, period_start)
               DO UPDATE SET
                 total_tx_count = EXCLUDED.total_tx_count,
                 total_cngn_sent = EXCLUDED.total_cngn_sent,
                 total_cngn_received = EXCLUDED.total_cngn_received,
                 total_fiat_onramped = EXCLUDED.total_fiat_onramped,
                 total_fiat_offramped = EXCLUDED.total_fiat_offramped,
                 total_fees_paid = EXCLUDED.total_fees_paid,
                 unique_counterparties = EXCLUDED.unique_counterparties,
                 most_used_tx_type = EXCLUDED.most_used_tx_type,
                 most_used_provider = EXCLUDED.most_used_provider,
                 active_days = EXCLUDED.active_days,
                 snapshot_at = now(),
                 updated_at = now()
               RETURNING id, wallet_address, period, period_start, period_end,
                 total_tx_count, total_cngn_sent, total_cngn_received,
                 total_fiat_onramped, total_fiat_offramped, total_fees_paid,
                 unique_counterparties, most_used_tx_type, most_used_provider,
                 active_days, snapshot_at"#,
        )
        .bind(&s.wallet_address)
        .bind(&s.period)
        .bind(s.period_start)
        .bind(s.period_end)
        .bind(s.total_tx_count)
        .bind(s.total_cngn_sent)
        .bind(s.total_cngn_received)
        .bind(s.total_fiat_onramped)
        .bind(s.total_fiat_offramped)
        .bind(s.total_fees_paid)
        .bind(s.unique_counterparties)
        .bind(s.most_used_tx_type)
        .bind(s.most_used_provider)
        .bind(s.active_days)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_snapshots(
        &self,
        wallet_address: &str,
        period: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<SnapshotRow>, DatabaseError> {
        sqlx::query_as::<_, SnapshotRow>(
            r#"SELECT id, wallet_address, period, period_start, period_end,
                 total_tx_count, total_cngn_sent, total_cngn_received,
                 total_fiat_onramped, total_fiat_offramped, total_fees_paid,
                 unique_counterparties, most_used_tx_type, most_used_provider,
                 active_days, snapshot_at
               FROM wallet_usage_snapshots
               WHERE wallet_address = $1 AND period = $2
                 AND period_start >= $3 AND period_start <= $4
               ORDER BY period_start DESC"#,
        )
        .bind(wallet_address)
        .bind(period)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_latest_snapshot_at(
        &self,
        wallet_address: &str,
        period: &str,
    ) -> Result<Option<DateTime<Utc>>, DatabaseError> {
        let row: Option<(DateTime<Utc>,)> = sqlx::query_as(
            "SELECT snapshot_at FROM wallet_usage_snapshots
             WHERE wallet_address = $1 AND period = $2
             ORDER BY period_start DESC LIMIT 1",
        )
        .bind(wallet_address)
        .bind(period)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(row.map(|(t,)| t))
    }

    /// All distinct wallet addresses that have had transactions since `since`.
    pub async fn active_wallet_addresses(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<String>, DatabaseError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT wallet_address FROM transactions WHERE created_at >= $1",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(rows.into_iter().map(|(a,)| a).collect())
    }

    // --- Spending categories ---

    pub async fn upsert_spending_category(
        &self,
        wallet_address: &str,
        period: &str,
        period_start: DateTime<Utc>,
        category: &str,
        tx_count: i32,
        total_amount: sqlx::types::BigDecimal,
        percentage: sqlx::types::BigDecimal,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO wallet_spending_categories
               (wallet_address, period, period_start, category, tx_count, total_amount, percentage)
               VALUES ($1,$2,$3,$4,$5,$6,$7)
               ON CONFLICT (wallet_address, period, period_start, category)
               DO UPDATE SET tx_count=EXCLUDED.tx_count,
                 total_amount=EXCLUDED.total_amount,
                 percentage=EXCLUDED.percentage,
                 updated_at=now()"#,
        )
        .bind(wallet_address)
        .bind(period)
        .bind(period_start)
        .bind(category)
        .bind(tx_count)
        .bind(total_amount)
        .bind(percentage)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn get_spending_categories(
        &self,
        wallet_address: &str,
        period: &str,
        period_start: DateTime<Utc>,
    ) -> Result<Vec<SpendingCategoryRow>, DatabaseError> {
        sqlx::query_as::<_, SpendingCategoryRow>(
            r#"SELECT id, wallet_address, period, period_start, category,
                 tx_count, total_amount, percentage
               FROM wallet_spending_categories
               WHERE wallet_address=$1 AND period=$2 AND period_start=$3
               ORDER BY total_amount DESC"#,
        )
        .bind(wallet_address)
        .bind(period)
        .bind(period_start)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // --- Counterparties ---

    pub async fn upsert_counterparty(
        &self,
        wallet_address: &str,
        counterparty_id: &str,
        counterparty_type: &str,
        amount: sqlx::types::BigDecimal,
        tx_at: DateTime<Utc>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO wallet_counterparty_frequency
               (wallet_address, counterparty_id, counterparty_type, tx_count, total_amount_sent, first_tx_at, last_tx_at)
               VALUES ($1,$2,$3,1,$4,$5,$5)
               ON CONFLICT (wallet_address, counterparty_id)
               DO UPDATE SET
                 tx_count = wallet_counterparty_frequency.tx_count + 1,
                 total_amount_sent = wallet_counterparty_frequency.total_amount_sent + EXCLUDED.total_amount_sent,
                 last_tx_at = EXCLUDED.last_tx_at,
                 updated_at = now()"#,
        )
        .bind(wallet_address)
        .bind(counterparty_id)
        .bind(counterparty_type)
        .bind(amount)
        .bind(tx_at)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn get_top_counterparties(
        &self,
        wallet_address: &str,
        limit: i64,
    ) -> Result<Vec<CounterpartyRow>, DatabaseError> {
        sqlx::query_as::<_, CounterpartyRow>(
            r#"SELECT id, wallet_address, counterparty_id, counterparty_type,
                 tx_count, total_amount_sent, first_tx_at, last_tx_at
               FROM wallet_counterparty_frequency
               WHERE wallet_address=$1
               ORDER BY tx_count DESC, total_amount_sent DESC
               LIMIT $2"#,
        )
        .bind(wallet_address)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // --- Behaviour profiles ---

    pub async fn upsert_profile(&self, p: UpsertProfile) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO wallet_behaviour_profiles
               (wallet_address, avg_tx_size, tx_frequency_per_week, preferred_hour_utc,
                preferred_provider, preferred_currency_pair, risk_score, profile_updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,now())
               ON CONFLICT (wallet_address)
               DO UPDATE SET
                 avg_tx_size=EXCLUDED.avg_tx_size,
                 tx_frequency_per_week=EXCLUDED.tx_frequency_per_week,
                 preferred_hour_utc=EXCLUDED.preferred_hour_utc,
                 preferred_provider=EXCLUDED.preferred_provider,
                 preferred_currency_pair=EXCLUDED.preferred_currency_pair,
                 risk_score=EXCLUDED.risk_score,
                 profile_updated_at=now(),
                 updated_at=now()"#,
        )
        .bind(&p.wallet_address)
        .bind(p.avg_tx_size)
        .bind(p.tx_frequency_per_week)
        .bind(p.preferred_hour_utc)
        .bind(p.preferred_provider)
        .bind(p.preferred_currency_pair)
        .bind(p.risk_score)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn get_profile(
        &self,
        wallet_address: &str,
    ) -> Result<Option<BehaviourProfileRow>, DatabaseError> {
        sqlx::query_as::<_, BehaviourProfileRow>(
            r#"SELECT id, wallet_address, avg_tx_size, tx_frequency_per_week,
                 preferred_hour_utc, preferred_provider, preferred_currency_pair,
                 risk_score, profile_updated_at
               FROM wallet_behaviour_profiles WHERE wallet_address=$1"#,
        )
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_all_profiles(&self) -> Result<Vec<BehaviourProfileRow>, DatabaseError> {
        sqlx::query_as::<_, BehaviourProfileRow>(
            r#"SELECT id, wallet_address, avg_tx_size, tx_frequency_per_week,
                 preferred_hour_utc, preferred_provider, preferred_currency_pair,
                 risk_score, profile_updated_at
               FROM wallet_behaviour_profiles"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // --- Insights ---

    pub async fn upsert_insight(
        &self,
        wallet_address: &str,
        period: &str,
        period_start: DateTime<Utc>,
        top_category: Option<&str>,
        top_category_amount: Option<sqlx::types::BigDecimal>,
        prev_period_delta_pct: Option<sqlx::types::BigDecimal>,
        largest_tx_amount: Option<sqlx::types::BigDecimal>,
        largest_tx_id: Option<Uuid>,
        most_frequent_counterparty: Option<&str>,
        estimated_monthly_fees: Option<sqlx::types::BigDecimal>,
        cngn_balance_trend: Option<&str>,
    ) -> Result<InsightRow, DatabaseError> {
        sqlx::query_as::<_, InsightRow>(
            r#"INSERT INTO wallet_insights
               (wallet_address, period, period_start, top_category, top_category_amount,
                prev_period_delta_pct, largest_tx_amount, largest_tx_id,
                most_frequent_counterparty, estimated_monthly_fees, cngn_balance_trend)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
               ON CONFLICT (wallet_address, period, period_start)
               DO UPDATE SET
                 top_category=EXCLUDED.top_category,
                 top_category_amount=EXCLUDED.top_category_amount,
                 prev_period_delta_pct=EXCLUDED.prev_period_delta_pct,
                 largest_tx_amount=EXCLUDED.largest_tx_amount,
                 largest_tx_id=EXCLUDED.largest_tx_id,
                 most_frequent_counterparty=EXCLUDED.most_frequent_counterparty,
                 estimated_monthly_fees=EXCLUDED.estimated_monthly_fees,
                 cngn_balance_trend=EXCLUDED.cngn_balance_trend,
                 generated_at=now()
               RETURNING id, wallet_address, period, period_start, top_category,
                 top_category_amount, prev_period_delta_pct, largest_tx_amount,
                 largest_tx_id, most_frequent_counterparty, estimated_monthly_fees,
                 cngn_balance_trend, generated_at, delivered_at"#,
        )
        .bind(wallet_address)
        .bind(period)
        .bind(period_start)
        .bind(top_category)
        .bind(top_category_amount)
        .bind(prev_period_delta_pct)
        .bind(largest_tx_amount)
        .bind(largest_tx_id)
        .bind(most_frequent_counterparty)
        .bind(estimated_monthly_fees)
        .bind(cngn_balance_trend)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_latest_insights(
        &self,
        wallet_address: &str,
        limit: i64,
    ) -> Result<Vec<InsightRow>, DatabaseError> {
        sqlx::query_as::<_, InsightRow>(
            r#"SELECT id, wallet_address, period, period_start, top_category,
                 top_category_amount, prev_period_delta_pct, largest_tx_amount,
                 largest_tx_id, most_frequent_counterparty, estimated_monthly_fees,
                 cngn_balance_trend, generated_at, delivered_at
               FROM wallet_insights WHERE wallet_address=$1
               ORDER BY generated_at DESC LIMIT $2"#,
        )
        .bind(wallet_address)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_insight_preferences(
        &self,
        wallet_address: &str,
    ) -> Result<Option<(bool, bool)>, DatabaseError> {
        let row: Option<(bool, bool)> = sqlx::query_as(
            "SELECT weekly_insights, monthly_insights FROM wallet_insight_preferences WHERE wallet_address=$1",
        )
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(row)
    }

    pub async fn upsert_insight_preferences(
        &self,
        wallet_address: &str,
        weekly: bool,
        monthly: bool,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO wallet_insight_preferences (wallet_address, weekly_insights, monthly_insights)
               VALUES ($1,$2,$3)
               ON CONFLICT (wallet_address)
               DO UPDATE SET weekly_insights=EXCLUDED.weekly_insights,
                 monthly_insights=EXCLUDED.monthly_insights, updated_at=now()"#,
        )
        .bind(wallet_address)
        .bind(weekly)
        .bind(monthly)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // --- Anomaly flags ---

    pub async fn insert_anomaly(
        &self,
        wallet_address: &str,
        anomaly_type: &str,
        deviation_magnitude: sqlx::types::BigDecimal,
    ) -> Result<AnomalyFlagRow, DatabaseError> {
        sqlx::query_as::<_, AnomalyFlagRow>(
            r#"INSERT INTO wallet_anomaly_flags
               (wallet_address, anomaly_type, deviation_magnitude)
               VALUES ($1,$2,$3)
               RETURNING id, wallet_address, anomaly_type, deviation_magnitude,
                 flagged_at, resolved_at, routed_to_compliance, compliance_case_id"#,
        )
        .bind(wallet_address)
        .bind(anomaly_type)
        .bind(deviation_magnitude)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_open_anomalies(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AnomalyFlagRow>, DatabaseError> {
        sqlx::query_as::<_, AnomalyFlagRow>(
            r#"SELECT id, wallet_address, anomaly_type, deviation_magnitude,
                 flagged_at, resolved_at, routed_to_compliance, compliance_case_id
               FROM wallet_anomaly_flags
               WHERE resolved_at IS NULL
               ORDER BY flagged_at DESC
               LIMIT $1 OFFSET $2"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn count_open_anomalies(&self) -> Result<i64, DatabaseError> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM wallet_anomaly_flags WHERE resolved_at IS NULL")
                .fetch_one(&self.pool)
                .await
                .map_err(DatabaseError::from_sqlx)?;
        Ok(count)
    }

    pub async fn mark_anomaly_routed(&self, id: Uuid, case_id: &str) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE wallet_anomaly_flags SET routed_to_compliance=true, compliance_case_id=$2, updated_at=now() WHERE id=$1",
        )
        .bind(id)
        .bind(case_id)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // --- Admin daily aggregates ---

    pub async fn upsert_daily_aggregate(
        &self,
        agg_date: chrono::NaiveDate,
        total_wallets: i64,
        active_wallets: i64,
        new_wallets: i64,
        total_cngn_transferred: sqlx::types::BigDecimal,
        total_fiat_onramped: sqlx::types::BigDecimal,
        total_fiat_offramped: sqlx::types::BigDecimal,
        avg_tx_size: sqlx::types::BigDecimal,
        total_tx_count: i64,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO admin_daily_aggregates
               (agg_date, total_wallets, active_wallets, new_wallets,
                total_cngn_transferred, total_fiat_onramped, total_fiat_offramped,
                avg_tx_size, total_tx_count)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               ON CONFLICT (agg_date)
               DO UPDATE SET
                 total_wallets=EXCLUDED.total_wallets,
                 active_wallets=EXCLUDED.active_wallets,
                 new_wallets=EXCLUDED.new_wallets,
                 total_cngn_transferred=EXCLUDED.total_cngn_transferred,
                 total_fiat_onramped=EXCLUDED.total_fiat_onramped,
                 total_fiat_offramped=EXCLUDED.total_fiat_offramped,
                 avg_tx_size=EXCLUDED.avg_tx_size,
                 total_tx_count=EXCLUDED.total_tx_count,
                 computed_at=now()"#,
        )
        .bind(agg_date)
        .bind(total_wallets)
        .bind(active_wallets)
        .bind(new_wallets)
        .bind(total_cngn_transferred)
        .bind(total_fiat_onramped)
        .bind(total_fiat_offramped)
        .bind(avg_tx_size)
        .bind(total_tx_count)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn get_daily_aggregates(
        &self,
        from: chrono::NaiveDate,
        to: chrono::NaiveDate,
    ) -> Result<Vec<AdminDailyAggRow>, DatabaseError> {
        sqlx::query_as::<_, AdminDailyAggRow>(
            r#"SELECT id, agg_date, total_wallets, active_wallets, new_wallets,
                 total_cngn_transferred, total_fiat_onramped, total_fiat_offramped,
                 avg_tx_size, total_tx_count, computed_at
               FROM admin_daily_aggregates
               WHERE agg_date >= $1 AND agg_date <= $2
               ORDER BY agg_date DESC"#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Risk score distribution across all profiles.
    pub async fn risk_score_distribution(&self) -> Result<Vec<(String, i64, f64, f64)>, DatabaseError> {
        // Returns (band_label, count, min_score, max_score)
        let rows: Vec<(String, i64, sqlx::types::BigDecimal, sqlx::types::BigDecimal)> =
            sqlx::query_as(
                r#"SELECT
                     CASE
                       WHEN risk_score < 25 THEN 'low'
                       WHEN risk_score < 50 THEN 'medium'
                       WHEN risk_score < 75 THEN 'high'
                       ELSE 'critical'
                     END AS band,
                     COUNT(*) AS cnt,
                     MIN(risk_score) AS min_score,
                     MAX(risk_score) AS max_score
                   FROM wallet_behaviour_profiles
                   GROUP BY band
                   ORDER BY min_score"#,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(DatabaseError::from_sqlx)?;

        use bigdecimal::ToPrimitive;
        Ok(rows
            .into_iter()
            .map(|(band, cnt, min_s, max_s)| {
                (band, cnt, min_s.to_f64().unwrap_or(0.0), max_s.to_f64().unwrap_or(0.0))
            })
            .collect())
    }

    pub async fn avg_risk_score(&self) -> Result<f64, DatabaseError> {
        let row: Option<(sqlx::types::BigDecimal,)> =
            sqlx::query_as("SELECT AVG(risk_score) FROM wallet_behaviour_profiles")
                .fetch_optional(&self.pool)
                .await
                .map_err(DatabaseError::from_sqlx)?;
        use bigdecimal::ToPrimitive;
        Ok(row.and_then(|(v,)| v.to_f64()).unwrap_or(0.0))
    }

    /// Cohort analysis: group wallets by registration month, count active in a period.
    pub async fn cohort_analysis(
        &self,
        active_from: DateTime<Utc>,
        active_to: DateTime<Utc>,
    ) -> Result<Vec<(String, i64, i64)>, DatabaseError> {
        // Returns (cohort_month, cohort_size, active_in_period)
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            r#"SELECT
                 TO_CHAR(DATE_TRUNC('month', w.created_at), 'YYYY-MM') AS cohort_month,
                 COUNT(DISTINCT w.wallet_address) AS cohort_size,
                 COUNT(DISTINCT t.wallet_address) AS active_in_period
               FROM wallets w
               LEFT JOIN transactions t
                 ON t.wallet_address = w.wallet_address
                 AND t.created_at >= $1 AND t.created_at <= $2
               GROUP BY cohort_month
               ORDER BY cohort_month DESC
               LIMIT 24"#,
        )
        .bind(active_from)
        .bind(active_to)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(rows)
    }

    /// Retention: wallets active in both current and previous period.
    pub async fn retention_metrics(
        &self,
        prev_from: DateTime<Utc>,
        prev_to: DateTime<Utc>,
        curr_from: DateTime<Utc>,
        curr_to: DateTime<Utc>,
    ) -> Result<(i64, i64), DatabaseError> {
        // Returns (retained, churned)
        let (retained,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(DISTINCT prev.wallet_address)
               FROM (SELECT DISTINCT wallet_address FROM transactions WHERE created_at >= $1 AND created_at <= $2) prev
               JOIN (SELECT DISTINCT wallet_address FROM transactions WHERE created_at >= $3 AND created_at <= $4) curr
                 USING (wallet_address)"#,
        )
        .bind(prev_from)
        .bind(prev_to)
        .bind(curr_from)
        .bind(curr_to)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        let (prev_total,): (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT wallet_address) FROM transactions WHERE created_at >= $1 AND created_at <= $2",
        )
        .bind(prev_from)
        .bind(prev_to)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok((retained, (prev_total - retained).max(0)))
    }
}
