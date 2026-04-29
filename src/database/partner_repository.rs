//! Database access layer for remittance partner entities (Issue #408).

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{types::BigDecimal, FromRow, PgPool};
use uuid::Uuid;

use crate::database::error::DatabaseError;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow)]
pub struct PartnerRow {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub status: String,
    pub api_key_hash: String,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PartnerBrandingRow {
    pub partner_id: Uuid,
    pub logo_url: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub email_template: serde_json::Value,
    pub language_overrides: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PartnerFeeRow {
    pub id: Uuid,
    pub partner_id: Uuid,
    pub corridor: String,
    pub fee_type: String,
    pub fee_value: BigDecimal,
    pub min_amount: Option<BigDecimal>,
    pub max_amount: Option<BigDecimal>,
    pub is_active: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct PartnerLimitsRow {
    pub partner_id: Uuid,
    pub daily_volume_limit: Option<BigDecimal>,
    pub per_tx_min: BigDecimal,
    pub per_tx_max: Option<BigDecimal>,
    pub kyc_threshold: Option<BigDecimal>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LiquidityAccountRow {
    pub id: Uuid,
    pub partner_id: Uuid,
    pub currency: String,
    pub stellar_address: Option<String>,
    pub balance: BigDecimal,
    pub reserved: BigDecimal,
}

#[derive(Debug, Clone, FromRow)]
pub struct PartnerTransferRow {
    pub id: Uuid,
    pub partner_id: Uuid,
    pub partner_ref: String,
    pub from_currency: String,
    pub to_currency: String,
    pub from_amount: BigDecimal,
    pub to_amount: BigDecimal,
    pub fee_amount: BigDecimal,
    pub fx_rate: BigDecimal,
    pub status: String,
    pub stellar_tx_hash: Option<String>,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PartnerSettlementRow {
    pub id: Uuid,
    pub partner_id: Uuid,
    pub settlement_date: NaiveDate,
    pub total_volume: BigDecimal,
    pub total_fees: BigDecimal,
    pub net_payable: BigDecimal,
    pub tx_count: i32,
    pub status: String,
    pub report_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Repository
// ---------------------------------------------------------------------------

pub struct PartnerRepository {
    pool: PgPool,
}

impl PartnerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // --- Partner CRUD ---

    pub async fn find_by_api_key_hash(&self, hash: &str) -> Result<Option<PartnerRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerRow>(
            "SELECT id,slug,name,status,api_key_hash,webhook_url,webhook_secret,created_at,updated_at
             FROM remittance_partners WHERE api_key_hash=$1 AND status='active'",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<PartnerRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerRow>(
            "SELECT id,slug,name,status,api_key_hash,webhook_url,webhook_secret,created_at,updated_at
             FROM remittance_partners WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_partners(&self) -> Result<Vec<PartnerRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerRow>(
            "SELECT id,slug,name,status,api_key_hash,webhook_url,webhook_secret,created_at,updated_at
             FROM remittance_partners ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn create_partner(
        &self,
        slug: &str,
        name: &str,
        api_key_hash: &str,
        webhook_url: Option<&str>,
        webhook_secret: Option<&str>,
    ) -> Result<PartnerRow, DatabaseError> {
        sqlx::query_as::<_, PartnerRow>(
            r#"INSERT INTO remittance_partners (slug,name,api_key_hash,webhook_url,webhook_secret)
               VALUES ($1,$2,$3,$4,$5)
               RETURNING id,slug,name,status,api_key_hash,webhook_url,webhook_secret,created_at,updated_at"#,
        )
        .bind(slug).bind(name).bind(api_key_hash).bind(webhook_url).bind(webhook_secret)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn update_partner_status(&self, id: Uuid, status: &str) -> Result<(), DatabaseError> {
        sqlx::query("UPDATE remittance_partners SET status=$2,updated_at=now() WHERE id=$1")
            .bind(id).bind(status)
            .execute(&self.pool).await.map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // --- Branding ---

    pub async fn get_branding(&self, partner_id: Uuid) -> Result<Option<PartnerBrandingRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerBrandingRow>(
            "SELECT partner_id,logo_url,primary_color,secondary_color,email_template,language_overrides,updated_at
             FROM partner_branding WHERE partner_id=$1",
        )
        .bind(partner_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn upsert_branding(
        &self,
        partner_id: Uuid,
        logo_url: Option<&str>,
        primary_color: Option<&str>,
        secondary_color: Option<&str>,
        email_template: serde_json::Value,
        language_overrides: serde_json::Value,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO partner_branding (partner_id,logo_url,primary_color,secondary_color,email_template,language_overrides)
               VALUES ($1,$2,$3,$4,$5,$6)
               ON CONFLICT (partner_id) DO UPDATE SET
                 logo_url=EXCLUDED.logo_url, primary_color=EXCLUDED.primary_color,
                 secondary_color=EXCLUDED.secondary_color, email_template=EXCLUDED.email_template,
                 language_overrides=EXCLUDED.language_overrides, updated_at=now()"#,
        )
        .bind(partner_id).bind(logo_url).bind(primary_color).bind(secondary_color)
        .bind(email_template).bind(language_overrides)
        .execute(&self.pool).await.map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // --- Fee structures ---

    pub async fn get_fee(&self, partner_id: Uuid, corridor: &str) -> Result<Option<PartnerFeeRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerFeeRow>(
            "SELECT id,partner_id,corridor,fee_type,fee_value,min_amount,max_amount,is_active
             FROM partner_fee_structures WHERE partner_id=$1 AND corridor=$2 AND is_active=true",
        )
        .bind(partner_id).bind(corridor)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_fees(&self, partner_id: Uuid) -> Result<Vec<PartnerFeeRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerFeeRow>(
            "SELECT id,partner_id,corridor,fee_type,fee_value,min_amount,max_amount,is_active
             FROM partner_fee_structures WHERE partner_id=$1 ORDER BY corridor",
        )
        .bind(partner_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn upsert_fee(
        &self,
        partner_id: Uuid,
        corridor: &str,
        fee_type: &str,
        fee_value: BigDecimal,
        min_amount: Option<BigDecimal>,
        max_amount: Option<BigDecimal>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO partner_fee_structures (partner_id,corridor,fee_type,fee_value,min_amount,max_amount)
               VALUES ($1,$2,$3,$4,$5,$6)
               ON CONFLICT (partner_id,corridor) DO UPDATE SET
                 fee_type=EXCLUDED.fee_type, fee_value=EXCLUDED.fee_value,
                 min_amount=EXCLUDED.min_amount, max_amount=EXCLUDED.max_amount,
                 is_active=true, updated_at=now()"#,
        )
        .bind(partner_id).bind(corridor).bind(fee_type).bind(fee_value)
        .bind(min_amount).bind(max_amount)
        .execute(&self.pool).await.map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // --- Limits ---

    pub async fn get_limits(&self, partner_id: Uuid) -> Result<Option<PartnerLimitsRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerLimitsRow>(
            "SELECT partner_id,daily_volume_limit,per_tx_min,per_tx_max,kyc_threshold
             FROM partner_limits WHERE partner_id=$1",
        )
        .bind(partner_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn upsert_limits(
        &self,
        partner_id: Uuid,
        daily_volume_limit: Option<BigDecimal>,
        per_tx_min: BigDecimal,
        per_tx_max: Option<BigDecimal>,
        kyc_threshold: Option<BigDecimal>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO partner_limits (partner_id,daily_volume_limit,per_tx_min,per_tx_max,kyc_threshold)
               VALUES ($1,$2,$3,$4,$5)
               ON CONFLICT (partner_id) DO UPDATE SET
                 daily_volume_limit=EXCLUDED.daily_volume_limit, per_tx_min=EXCLUDED.per_tx_min,
                 per_tx_max=EXCLUDED.per_tx_max, kyc_threshold=EXCLUDED.kyc_threshold, updated_at=now()"#,
        )
        .bind(partner_id).bind(daily_volume_limit).bind(per_tx_min).bind(per_tx_max).bind(kyc_threshold)
        .execute(&self.pool).await.map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // --- Liquidity accounts ---

    pub async fn get_liquidity(&self, partner_id: Uuid, currency: &str) -> Result<Option<LiquidityAccountRow>, DatabaseError> {
        sqlx::query_as::<_, LiquidityAccountRow>(
            "SELECT id,partner_id,currency,stellar_address,balance,reserved
             FROM partner_liquidity_accounts WHERE partner_id=$1 AND currency=$2",
        )
        .bind(partner_id).bind(currency)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_liquidity(&self, partner_id: Uuid) -> Result<Vec<LiquidityAccountRow>, DatabaseError> {
        sqlx::query_as::<_, LiquidityAccountRow>(
            "SELECT id,partner_id,currency,stellar_address,balance,reserved
             FROM partner_liquidity_accounts WHERE partner_id=$1",
        )
        .bind(partner_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn upsert_liquidity(
        &self,
        partner_id: Uuid,
        currency: &str,
        stellar_address: Option<&str>,
        balance: BigDecimal,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"INSERT INTO partner_liquidity_accounts (partner_id,currency,stellar_address,balance)
               VALUES ($1,$2,$3,$4)
               ON CONFLICT (partner_id,currency) DO UPDATE SET
                 stellar_address=EXCLUDED.stellar_address, balance=EXCLUDED.balance, updated_at=now()"#,
        )
        .bind(partner_id).bind(currency).bind(stellar_address).bind(balance)
        .execute(&self.pool).await.map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // --- Transfers ---

    pub async fn create_transfer(
        &self,
        partner_id: Uuid,
        partner_ref: &str,
        from_currency: &str,
        to_currency: &str,
        from_amount: BigDecimal,
        to_amount: BigDecimal,
        fee_amount: BigDecimal,
        fx_rate: BigDecimal,
        metadata: serde_json::Value,
    ) -> Result<PartnerTransferRow, DatabaseError> {
        sqlx::query_as::<_, PartnerTransferRow>(
            r#"INSERT INTO partner_transfers
               (partner_id,partner_ref,from_currency,to_currency,from_amount,to_amount,fee_amount,fx_rate,metadata)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               RETURNING id,partner_id,partner_ref,from_currency,to_currency,from_amount,to_amount,
                 fee_amount,fx_rate,status,stellar_tx_hash,error_message,metadata,created_at,updated_at"#,
        )
        .bind(partner_id).bind(partner_ref).bind(from_currency).bind(to_currency)
        .bind(from_amount).bind(to_amount).bind(fee_amount).bind(fx_rate).bind(metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_transfer(&self, id: Uuid, partner_id: Uuid) -> Result<Option<PartnerTransferRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerTransferRow>(
            r#"SELECT id,partner_id,partner_ref,from_currency,to_currency,from_amount,to_amount,
                 fee_amount,fx_rate,status,stellar_tx_hash,error_message,metadata,created_at,updated_at
               FROM partner_transfers WHERE id=$1 AND partner_id=$2"#,
        )
        .bind(id).bind(partner_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn update_transfer_status(
        &self,
        id: Uuid,
        status: &str,
        stellar_tx_hash: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE partner_transfers SET status=$2,stellar_tx_hash=$3,error_message=$4,updated_at=now() WHERE id=$1",
        )
        .bind(id).bind(status).bind(stellar_tx_hash).bind(error_message)
        .execute(&self.pool).await.map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    /// Sum completed transfers for a partner on a given date (for settlement).
    pub async fn daily_transfer_summary(
        &self,
        partner_id: Uuid,
        date: NaiveDate,
    ) -> Result<(BigDecimal, BigDecimal, i64), DatabaseError> {
        // Returns (total_volume, total_fees, tx_count)
        let row: (Option<BigDecimal>, Option<BigDecimal>, i64) = sqlx::query_as(
            r#"SELECT SUM(from_amount), SUM(fee_amount), COUNT(*)
               FROM partner_transfers
               WHERE partner_id=$1 AND status='completed'
                 AND DATE(created_at)=$2"#,
        )
        .bind(partner_id).bind(date)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok((
            row.0.unwrap_or_default(),
            row.1.unwrap_or_default(),
            row.2,
        ))
    }

    // --- Settlements ---

    pub async fn upsert_settlement(
        &self,
        partner_id: Uuid,
        settlement_date: NaiveDate,
        total_volume: BigDecimal,
        total_fees: BigDecimal,
        net_payable: BigDecimal,
        tx_count: i64,
    ) -> Result<PartnerSettlementRow, DatabaseError> {
        sqlx::query_as::<_, PartnerSettlementRow>(
            r#"INSERT INTO partner_settlements
               (partner_id,settlement_date,total_volume,total_fees,net_payable,tx_count)
               VALUES ($1,$2,$3,$4,$5,$6)
               ON CONFLICT (partner_id,settlement_date) DO UPDATE SET
                 total_volume=EXCLUDED.total_volume, total_fees=EXCLUDED.total_fees,
                 net_payable=EXCLUDED.net_payable, tx_count=EXCLUDED.tx_count, updated_at=now()
               RETURNING id,partner_id,settlement_date,total_volume,total_fees,net_payable,
                 tx_count,status,report_url,created_at"#,
        )
        .bind(partner_id).bind(settlement_date).bind(total_volume).bind(total_fees)
        .bind(net_payable).bind(tx_count as i32)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_settlements(&self, partner_id: Uuid, limit: i64) -> Result<Vec<PartnerSettlementRow>, DatabaseError> {
        sqlx::query_as::<_, PartnerSettlementRow>(
            r#"SELECT id,partner_id,settlement_date,total_volume,total_fees,net_payable,
                 tx_count,status,report_url,created_at
               FROM partner_settlements WHERE partner_id=$1
               ORDER BY settlement_date DESC LIMIT $2"#,
        )
        .bind(partner_id).bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn mark_settlement_sent(&self, id: Uuid, report_url: &str) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE partner_settlements SET status='sent',report_url=$2,updated_at=now() WHERE id=$1",
        )
        .bind(id).bind(report_url)
        .execute(&self.pool).await.map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }
}
