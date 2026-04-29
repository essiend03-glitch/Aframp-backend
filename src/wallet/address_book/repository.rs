use super::models::*;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

pub struct AddressBookRepository {
    pool: PgPool,
}

impl AddressBookRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new address book entry
    pub async fn create_entry(
        &self,
        owner_wallet_id: Uuid,
        entry_type: AddressEntryType,
        label: String,
        notes: Option<String>,
    ) -> Result<AddressBookEntry, sqlx::Error> {
        let entry = sqlx::query_as::<_, AddressBookEntry>(
            r#"
            INSERT INTO address_book_entries 
            (id, owner_wallet_id, entry_type, label, notes, entry_status, verification_status, use_count, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 0, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(owner_wallet_id)
        .bind(entry_type)
        .bind(label)
        .bind(notes)
        .bind(EntryStatus::Active)
        .bind(VerificationStatus::Pending)
        .fetch_one(&self.pool)
        .await?;

        Ok(entry)
    }

    /// Create Stellar wallet entry details
    pub async fn create_stellar_wallet_entry(
        &self,
        entry_id: Uuid,
        stellar_public_key: String,
        network: String,
    ) -> Result<StellarWalletEntry, sqlx::Error> {
        let stellar_entry = sqlx::query_as::<_, StellarWalletEntry>(
            r#"
            INSERT INTO stellar_wallet_entries
            (entry_id, stellar_public_key, network, account_exists_on_stellar, cngn_trustline_active, created_at, updated_at)
            VALUES ($1, $2, $3, false, false, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(stellar_public_key)
        .bind(network)
        .fetch_one(&self.pool)
        .await?;

        Ok(stellar_entry)
    }

    /// Create mobile money entry details
    pub async fn create_mobile_money_entry(
        &self,
        entry_id: Uuid,
        provider_name: String,
        phone_number: String,
        country_code: String,
    ) -> Result<MobileMoneyEntry, sqlx::Error> {
        let mobile_entry = sqlx::query_as::<_, MobileMoneyEntry>(
            r#"
            INSERT INTO mobile_money_entries
            (entry_id, provider_name, phone_number, country_code, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(provider_name)
        .bind(phone_number)
        .bind(country_code)
        .fetch_one(&self.pool)
        .await?;

        Ok(mobile_entry)
    }

    /// Create bank account entry details
    pub async fn create_bank_account_entry(
        &self,
        entry_id: Uuid,
        bank_name: String,
        account_number: String,
        sort_code: Option<String>,
        routing_number: Option<String>,
        country_code: String,
        currency: String,
    ) -> Result<BankAccountEntry, sqlx::Error> {
        let bank_entry = sqlx::query_as::<_, BankAccountEntry>(
            r#"
            INSERT INTO bank_account_entries
            (entry_id, bank_name, account_number, sort_code, routing_number, country_code, currency, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(bank_name)
        .bind(account_number)
        .bind(sort_code)
        .bind(routing_number)
        .bind(country_code)
        .bind(currency)
        .fetch_one(&self.pool)
        .await?;

        Ok(bank_entry)
    }

    /// Get address book entry by ID
    pub async fn get_entry(&self, entry_id: Uuid, owner_wallet_id: Uuid) -> Result<Option<AddressBookEntry>, sqlx::Error> {
        let entry = sqlx::query_as::<_, AddressBookEntry>(
            r#"
            SELECT * FROM address_book_entries
            WHERE id = $1 AND owner_wallet_id = $2 AND entry_status = 'active'
            "#,
        )
        .bind(entry_id)
        .bind(owner_wallet_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(entry)
    }

    /// Get Stellar wallet entry details
    pub async fn get_stellar_wallet_entry(&self, entry_id: Uuid) -> Result<Option<StellarWalletEntry>, sqlx::Error> {
        let entry = sqlx::query_as::<_, StellarWalletEntry>(
            "SELECT * FROM stellar_wallet_entries WHERE entry_id = $1",
        )
        .bind(entry_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(entry)
    }

    /// Get mobile money entry details
    pub async fn get_mobile_money_entry(&self, entry_id: Uuid) -> Result<Option<MobileMoneyEntry>, sqlx::Error> {
        let entry = sqlx::query_as::<_, MobileMoneyEntry>(
            "SELECT * FROM mobile_money_entries WHERE entry_id = $1",
        )
        .bind(entry_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(entry)
    }

    /// Get bank account entry details
    pub async fn get_bank_account_entry(&self, entry_id: Uuid) -> Result<Option<BankAccountEntry>, sqlx::Error> {
        let entry = sqlx::query_as::<_, BankAccountEntry>(
            "SELECT * FROM bank_account_entries WHERE entry_id = $1",
        )
        .bind(entry_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(entry)
    }

    /// List address book entries with filters
    pub async fn list_entries(
        &self,
        owner_wallet_id: Uuid,
        entry_type: Option<AddressEntryType>,
        group_id: Option<Uuid>,
        search: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AddressBookEntry>, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT DISTINCT e.* FROM address_book_entries e WHERE e.owner_wallet_id = "
        );
        query.push_bind(owner_wallet_id);
        query.push(" AND e.entry_status = 'active'");

        if let Some(et) = entry_type {
            query.push(" AND e.entry_type = ");
            query.push_bind(et);
        }

        if let Some(gid) = group_id {
            query.push(" AND e.id IN (SELECT entry_id FROM group_memberships WHERE group_id = ");
            query.push_bind(gid);
            query.push(")");
        }

        if let Some(search_term) = search {
            let search_pattern = format!("%{}%", search_term);
            query.push(" AND (e.label ILIKE ");
            query.push_bind(&search_pattern);
            query.push(" OR e.notes ILIKE ");
            query.push_bind(&search_pattern);
            query.push(")");
        }

        query.push(" ORDER BY e.last_used_at DESC NULLS LAST, e.created_at DESC");
        query.push(" LIMIT ");
        query.push_bind(limit);
        query.push(" OFFSET ");
        query.push_bind(offset);

        let entries = query
            .build_query_as::<AddressBookEntry>()
            .fetch_all(&self.pool)
            .await?;

        Ok(entries)
    }

    /// Count address book entries
    pub async fn count_entries(
        &self,
        owner_wallet_id: Uuid,
        entry_type: Option<AddressEntryType>,
        group_id: Option<Uuid>,
        search: Option<String>,
    ) -> Result<i64, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(DISTINCT e.id) FROM address_book_entries e WHERE e.owner_wallet_id = "
        );
        query.push_bind(owner_wallet_id);
        query.push(" AND e.entry_status = 'active'");

        if let Some(et) = entry_type {
            query.push(" AND e.entry_type = ");
            query.push_bind(et);
        }

        if let Some(gid) = group_id {
            query.push(" AND e.id IN (SELECT entry_id FROM group_memberships WHERE group_id = ");
            query.push_bind(gid);
            query.push(")");
        }

        if let Some(search_term) = search {
            let search_pattern = format!("%{}%", search_term);
            query.push(" AND (e.label ILIKE ");
            query.push_bind(&search_pattern);
            query.push(" OR e.notes ILIKE ");
            query.push_bind(&search_pattern);
            query.push(")");
        }

        let count: i64 = query
            .build()
            .fetch_one(&self.pool)
            .await?
            .try_get(0)?;

        Ok(count)
    }

    /// Update address book entry
    pub async fn update_entry(
        &self,
        entry_id: Uuid,
        owner_wallet_id: Uuid,
        label: Option<String>,
        notes: Option<String>,
    ) -> Result<AddressBookEntry, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new("UPDATE address_book_entries SET updated_at = NOW()");

        if let Some(l) = label {
            query.push(", label = ");
            query.push_bind(l);
        }

        if notes.is_some() {
            query.push(", notes = ");
            query.push_bind(notes);
        }

        query.push(" WHERE id = ");
        query.push_bind(entry_id);
        query.push(" AND owner_wallet_id = ");
        query.push_bind(owner_wallet_id);
        query.push(" AND entry_status = 'active' RETURNING *");

        let entry = query
            .build_query_as::<AddressBookEntry>()
            .fetch_one(&self.pool)
            .await?;

        Ok(entry)
    }

    /// Soft delete an entry
    pub async fn soft_delete_entry(
        &self,
        entry_id: Uuid,
        owner_wallet_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE address_book_entries
            SET entry_status = 'deleted', deleted_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND owner_wallet_id = $2 AND entry_status = 'active'
            "#,
        )
        .bind(entry_id)
        .bind(owner_wallet_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Restore a soft-deleted entry
    pub async fn restore_entry(
        &self,
        entry_id: Uuid,
        owner_wallet_id: Uuid,
    ) -> Result<AddressBookEntry, sqlx::Error> {
        let entry = sqlx::query_as::<_, AddressBookEntry>(
            r#"
            UPDATE address_book_entries
            SET entry_status = 'active', deleted_at = NULL, updated_at = NOW()
            WHERE id = $1 AND owner_wallet_id = $2 AND entry_status = 'deleted'
            AND deleted_at > NOW() - INTERVAL '30 days'
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(owner_wallet_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(entry)
    }

    /// Update entry usage statistics
    pub async fn update_entry_usage(&self, entry_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE address_book_entries
            SET last_used_at = NOW(), use_count = use_count + 1, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(entry_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update verification status
    pub async fn update_verification_status(
        &self,
        entry_id: Uuid,
        status: VerificationStatus,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE address_book_entries
            SET verification_status = $1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(status)
        .bind(entry_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update Stellar wallet verification
    pub async fn update_stellar_verification(
        &self,
        entry_id: Uuid,
        account_exists: bool,
        trustline_active: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE stellar_wallet_entries
            SET account_exists_on_stellar = $1, cngn_trustline_active = $2, 
                last_verified_at = NOW(), updated_at = NOW()
            WHERE entry_id = $3
            "#,
        )
        .bind(account_exists)
        .bind(trustline_active)
        .bind(entry_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update mobile money account name
    pub async fn update_mobile_money_account_name(
        &self,
        entry_id: Uuid,
        account_name: String,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE mobile_money_entries
            SET account_name = $1, last_verified_at = NOW(), updated_at = NOW()
            WHERE entry_id = $2
            "#,
        )
        .bind(account_name)
        .bind(entry_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update bank account name
    pub async fn update_bank_account_name(
        &self,
        entry_id: Uuid,
        account_name: String,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE bank_account_entries
            SET account_name = $1, last_verified_at = NOW(), updated_at = NOW()
            WHERE entry_id = $2
            "#,
        )
        .bind(account_name)
        .bind(entry_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Count entries by owner
    pub async fn count_entries_by_owner(&self, owner_wallet_id: Uuid) -> Result<i64, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM address_book_entries WHERE owner_wallet_id = $1 AND entry_status = 'active'",
        )
        .bind(owner_wallet_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Get stale entries for re-verification
    pub async fn get_stale_entries(&self, stale_threshold_hours: i64) -> Result<Vec<AddressBookEntry>, sqlx::Error> {
        let entries = sqlx::query_as::<_, AddressBookEntry>(
            r#"
            SELECT e.* FROM address_book_entries e
            WHERE e.entry_status = 'active'
            AND e.verification_status = 'verified'
            AND e.updated_at < NOW() - ($1 || ' hours')::INTERVAL
            ORDER BY e.updated_at ASC
            LIMIT 100
            "#,
        )
        .bind(stale_threshold_hours)
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Count stale verifications
    pub async fn count_stale_verifications(&self, stale_threshold_hours: i64) -> Result<i64, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM address_book_entries
            WHERE entry_status = 'active'
            AND verification_status = 'verified'
            AND updated_at < NOW() - ($1 || ' hours')::INTERVAL
            "#,
        )
        .bind(stale_threshold_hours)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Get recently and frequently used entries
    pub async fn get_suggested_entries(
        &self,
        owner_wallet_id: Uuid,
        entry_type: Option<AddressEntryType>,
        limit: i64,
    ) -> Result<Vec<AddressBookEntry>, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
            SELECT * FROM address_book_entries
            WHERE owner_wallet_id = 
            "#
        );
        query.push_bind(owner_wallet_id);
        query.push(" AND entry_status = 'active'");

        if let Some(et) = entry_type {
            query.push(" AND entry_type = ");
            query.push_bind(et);
        }

        query.push(
            r#"
            ORDER BY 
                CASE WHEN last_used_at > NOW() - INTERVAL '7 days' THEN 1 ELSE 2 END,
                use_count DESC,
                last_used_at DESC NULLS LAST
            LIMIT 
            "#
        );
        query.push_bind(limit);

        let entries = query
            .build_query_as::<AddressBookEntry>()
            .fetch_all(&self.pool)
            .await?;

        Ok(entries)
    }
}
