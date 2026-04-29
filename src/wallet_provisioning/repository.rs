//! Database access layer for wallet provisioning.

use crate::database::error::DatabaseError;
use crate::wallet_provisioning::models::*;
use sqlx::PgPool;
use uuid::Uuid;

pub struct ProvisioningRepository {
    pool: PgPool,
}

impl ProvisioningRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -------------------------------------------------------------------------
    // Provisioning state machine
    // -------------------------------------------------------------------------

    /// Create or return existing provisioning record (idempotent).
    pub async fn get_or_create(
        &self,
        wallet_id: Uuid,
    ) -> Result<WalletProvisioning, DatabaseError> {
        sqlx::query_as::<_, WalletProvisioning>(
            r#"
            INSERT INTO wallet_provisioning (wallet_id, state)
            VALUES ($1, 'keypair_generated')
            ON CONFLICT (wallet_id) DO UPDATE SET updated_at = wallet_provisioning.updated_at
            RETURNING *
            "#,
        )
        .bind(wallet_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get(&self, wallet_id: Uuid) -> Result<Option<WalletProvisioning>, DatabaseError> {
        sqlx::query_as::<_, WalletProvisioning>(
            "SELECT * FROM wallet_provisioning WHERE wallet_id = $1",
        )
        .bind(wallet_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Transition to a new state (idempotent — no-op if already in target state).
    pub async fn transition(
        &self,
        wallet_id: Uuid,
        new_state: &str,
        reason: Option<&str>,
        triggered_by: &str,
    ) -> Result<WalletProvisioning, DatabaseError> {
        // Record history
        sqlx::query(
            r#"
            INSERT INTO wallet_provisioning_history
                (wallet_id, from_state, to_state, transition_reason, triggered_by)
            SELECT wallet_id, state, $2, $3, $4
            FROM wallet_provisioning
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .bind(new_state)
        .bind(reason)
        .bind(triggered_by)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        // Update state
        sqlx::query_as::<_, WalletProvisioning>(
            r#"
            UPDATE wallet_provisioning
            SET state = $2, step_started_at = now(), updated_at = now()
            WHERE wallet_id = $1
            RETURNING *
            "#,
        )
        .bind(wallet_id)
        .bind(new_state)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn set_failure(
        &self,
        wallet_id: Uuid,
        reason: &str,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE wallet_provisioning
            SET state = 'failed',
                last_failure_reason = $2,
                last_failure_at = now(),
                retry_count = retry_count + 1,
                updated_at = now()
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .bind(reason)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn set_trustline_envelope(
        &self,
        wallet_id: Uuid,
        envelope_xdr: &str,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE wallet_provisioning
            SET trustline_envelope = $2, updated_at = now()
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .bind(envelope_xdr)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn set_trustline_submitted(
        &self,
        wallet_id: Uuid,
        tx_hash: &str,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE wallet_provisioning
            SET trustline_tx_hash = $2,
                trustline_submitted_at = now(),
                updated_at = now()
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .bind(tx_hash)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn set_funding_detected(
        &self,
        wallet_id: Uuid,
        tx_hash: Option<&str>,
        method: &str,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE wallet_provisioning
            SET funding_detected_at = now(),
                funding_tx_hash = $2,
                funding_method = $3,
                updated_at = now()
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .bind(tx_hash)
        .bind(method)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn set_ready(&self, wallet_id: Uuid) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE wallet_provisioning
            SET state = 'ready', became_ready_at = now(), updated_at = now()
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Readiness checks
    // -------------------------------------------------------------------------

    pub async fn upsert_readiness(
        &self,
        wallet_id: Uuid,
        stellar_exists: bool,
        min_xlm: bool,
        trustline_active: bool,
        trustline_authorized: bool,
        wallet_registered: bool,
    ) -> Result<WalletReadinessCheck, DatabaseError> {
        let all_met = stellar_exists && min_xlm && trustline_active && trustline_authorized && wallet_registered;
        sqlx::query_as::<_, WalletReadinessCheck>(
            r#"
            INSERT INTO wallet_readiness_checks
                (wallet_id, stellar_account_exists, min_xlm_balance_met,
                 trustline_active, trustline_authorized, wallet_registered,
                 all_criteria_met, checked_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,now())
            ON CONFLICT (wallet_id) DO UPDATE SET
                stellar_account_exists = EXCLUDED.stellar_account_exists,
                min_xlm_balance_met    = EXCLUDED.min_xlm_balance_met,
                trustline_active       = EXCLUDED.trustline_active,
                trustline_authorized   = EXCLUDED.trustline_authorized,
                wallet_registered      = EXCLUDED.wallet_registered,
                all_criteria_met       = EXCLUDED.all_criteria_met,
                checked_at             = now()
            RETURNING *
            "#,
        )
        .bind(wallet_id)
        .bind(stellar_exists)
        .bind(min_xlm)
        .bind(trustline_active)
        .bind(trustline_authorized)
        .bind(wallet_registered)
        .bind(all_met)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_readiness(
        &self,
        wallet_id: Uuid,
    ) -> Result<Option<WalletReadinessCheck>, DatabaseError> {
        sqlx::query_as::<_, WalletReadinessCheck>(
            "SELECT * FROM wallet_readiness_checks WHERE wallet_id = $1",
        )
        .bind(wallet_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // -------------------------------------------------------------------------
    // Platform funding account
    // -------------------------------------------------------------------------

    pub async fn get_funding_account(&self) -> Result<Option<PlatformFundingAccount>, DatabaseError> {
        sqlx::query_as::<_, PlatformFundingAccount>(
            "SELECT * FROM platform_funding_account WHERE is_active = TRUE LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn update_funding_account_balance(
        &self,
        account_id: Uuid,
        new_balance: f64,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE platform_funding_account
            SET current_xlm_balance = $2,
                last_balance_check_at = now(),
                updated_at = now()
            WHERE id = $1
            "#,
        )
        .bind(account_id)
        .bind(new_balance)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn record_sponsorship(
        &self,
        account_id: Uuid,
        xlm_spent: f64,
        tx_hash: &str,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE platform_funding_account
            SET total_accounts_sponsored = total_accounts_sponsored + 1,
                total_xlm_spent = total_xlm_spent + $2,
                current_xlm_balance = current_xlm_balance - $2,
                updated_at = now()
            WHERE id = $1
            "#,
        )
        .bind(account_id)
        .bind(xlm_spent)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        // Also update provisioning record
        sqlx::query(
            r#"
            UPDATE wallet_provisioning
            SET is_sponsored = TRUE,
                sponsorship_tx_hash = $1,
                sponsorship_xlm_amount = $2,
                funding_method = 'sponsored',
                updated_at = now()
            WHERE state = 'pending_funding'
              AND is_sponsored = FALSE
            "#,
        )
        .bind(tx_hash)
        .bind(xlm_spent)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(())
    }

    pub async fn create_replenishment_request(
        &self,
        funding_account_id: Uuid,
        requested_by: Option<Uuid>,
        amount: f64,
        notes: Option<&str>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO funding_account_replenishment_requests
                (funding_account_id, requested_by, requested_xlm_amount, notes)
            VALUES ($1,$2,$3,$4)
            "#,
        )
        .bind(funding_account_id)
        .bind(requested_by)
        .bind(amount)
        .bind(notes)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }
}
