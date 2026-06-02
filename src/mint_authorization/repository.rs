//! Database access layer for the Mint Authorization Framework.

use crate::mint_authorization::{
    error::MintAuthError,
    models::{MintAuthRequest, MintAuthSignature, MintAuthStatus},
};
use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use sqlx::PgPool;
use uuid::Uuid;

pub struct MintAuthRepository {
    pool: PgPool,
}

impl MintAuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Authorization requests
    // ─────────────────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn create_request(
        &self,
        amount_cngn: &BigDecimal,
        destination_account: &str,
        requested_by: Uuid,
        requested_by_key: &str,
        justification: &str,
        reserve_verification_id: Uuid,
        required_signatures: i16,
        unsigned_xdr: &str,
        tx_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<MintAuthRequest, MintAuthError> {
        sqlx::query_as!(
            MintAuthRequest,
            r#"
            INSERT INTO mint_authorization_requests (
                amount_cngn, destination_account, requested_by, requested_by_key,
                justification, reserve_verification_id, required_signatures,
                unsigned_xdr, tx_hash, expires_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            RETURNING
                id, amount_cngn, destination_account, requested_by, requested_by_key,
                justification, reserve_verification_id, required_signatures,
                collected_signatures, unsigned_xdr, signed_xdr, tx_hash,
                stellar_tx_hash, status AS "status: MintAuthStatus",
                failure_reason, cancellation_reason, cancelled_by, retry_count,
                expires_at, submitted_at, confirmed_at, created_at, updated_at
            "#,
            amount_cngn,
            destination_account,
            requested_by,
            requested_by_key,
            justification,
            reserve_verification_id,
            required_signatures,
            unsigned_xdr,
            tx_hash,
            expires_at,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(MintAuthError::from)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<MintAuthRequest>, MintAuthError> {
        sqlx::query_as!(
            MintAuthRequest,
            r#"
            SELECT id, amount_cngn, destination_account, requested_by, requested_by_key,
                   justification, reserve_verification_id, required_signatures,
                   collected_signatures, unsigned_xdr, signed_xdr, tx_hash,
                   stellar_tx_hash, status AS "status: MintAuthStatus",
                   failure_reason, cancellation_reason, cancelled_by, retry_count,
                   expires_at, submitted_at, confirmed_at, created_at, updated_at
            FROM mint_authorization_requests WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(MintAuthError::from)
    }

    pub async fn list(
        &self,
        status: Option<MintAuthStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<MintAuthRequest>, i64), MintAuthError> {
        let rows = sqlx::query_as!(
            MintAuthRequest,
            r#"
            SELECT id, amount_cngn, destination_account, requested_by, requested_by_key,
                   justification, reserve_verification_id, required_signatures,
                   collected_signatures, unsigned_xdr, signed_xdr, tx_hash,
                   stellar_tx_hash, status AS "status: MintAuthStatus",
                   failure_reason, cancellation_reason, cancelled_by, retry_count,
                   expires_at, submitted_at, confirmed_at, created_at, updated_at
            FROM mint_authorization_requests
            WHERE ($1::mint_auth_status IS NULL OR status = $1)
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            status as Option<MintAuthStatus>,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(MintAuthError::from)?;

        let total: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mint_authorization_requests
             WHERE ($1::mint_auth_status IS NULL OR status = $1)",
            status as Option<MintAuthStatus>,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(MintAuthError::from)?
        .unwrap_or(0);

        Ok((rows, total))
    }

    pub async fn update_status(
        &self,
        id: Uuid,
        status: MintAuthStatus,
        signed_xdr: Option<&str>,
        stellar_tx_hash: Option<&str>,
        failure_reason: Option<&str>,
    ) -> Result<MintAuthRequest, MintAuthError> {
        sqlx::query_as!(
            MintAuthRequest,
            r#"
            UPDATE mint_authorization_requests SET
                status = $2,
                signed_xdr = COALESCE($3, signed_xdr),
                stellar_tx_hash = COALESCE($4, stellar_tx_hash),
                failure_reason = COALESCE($5, failure_reason),
                submitted_at = CASE WHEN $2 = 'submitted' THEN NOW() ELSE submitted_at END,
                confirmed_at = CASE WHEN $2 = 'confirmed' THEN NOW() ELSE confirmed_at END
            WHERE id = $1
            RETURNING
                id, amount_cngn, destination_account, requested_by, requested_by_key,
                justification, reserve_verification_id, required_signatures,
                collected_signatures, unsigned_xdr, signed_xdr, tx_hash,
                stellar_tx_hash, status AS "status: MintAuthStatus",
                failure_reason, cancellation_reason, cancelled_by, retry_count,
                expires_at, submitted_at, confirmed_at, created_at, updated_at
            "#,
            id,
            status as MintAuthStatus,
            signed_xdr,
            stellar_tx_hash,
            failure_reason,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(MintAuthError::from)
    }

    pub async fn cancel(
        &self,
        id: Uuid,
        cancelled_by: Uuid,
        reason: &str,
    ) -> Result<MintAuthRequest, MintAuthError> {
        sqlx::query_as!(
            MintAuthRequest,
            r#"
            UPDATE mint_authorization_requests SET
                status = 'cancelled',
                cancellation_reason = $3,
                cancelled_by = $2
            WHERE id = $1
              AND status IN ('pending_signatures', 'threshold_met')
            RETURNING
                id, amount_cngn, destination_account, requested_by, requested_by_key,
                justification, reserve_verification_id, required_signatures,
                collected_signatures, unsigned_xdr, signed_xdr, tx_hash,
                stellar_tx_hash, status AS "status: MintAuthStatus",
                failure_reason, cancellation_reason, cancelled_by, retry_count,
                expires_at, submitted_at, confirmed_at, created_at, updated_at
            "#,
            id,
            cancelled_by,
            reason,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(MintAuthError::from)?
        .ok_or(MintAuthError::NotFound(id))
    }

    pub async fn increment_retry(&self, id: Uuid) -> Result<(), MintAuthError> {
        sqlx::query!(
            "UPDATE mint_authorization_requests SET retry_count = retry_count + 1 WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await
        .map_err(MintAuthError::from)?;
        Ok(())
    }

    /// Find all pending requests that have exceeded their expiry timestamp.
    pub async fn find_expired(&self) -> Result<Vec<MintAuthRequest>, MintAuthError> {
        sqlx::query_as!(
            MintAuthRequest,
            r#"
            SELECT id, amount_cngn, destination_account, requested_by, requested_by_key,
                   justification, reserve_verification_id, required_signatures,
                   collected_signatures, unsigned_xdr, signed_xdr, tx_hash,
                   stellar_tx_hash, status AS "status: MintAuthStatus",
                   failure_reason, cancellation_reason, cancelled_by, retry_count,
                   expires_at, submitted_at, confirmed_at, created_at, updated_at
            FROM mint_authorization_requests
            WHERE status = 'pending_signatures'
              AND expires_at < NOW()
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(MintAuthError::from)
    }

    /// Count of requests currently in pending_signatures status.
    pub async fn count_pending(&self) -> Result<i64, MintAuthError> {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mint_authorization_requests WHERE status = 'pending_signatures'"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(MintAuthError::from)
        .map(|c| c.unwrap_or(0))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Signatures
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn add_signature(
        &self,
        auth_request_id: Uuid,
        signer_id: Uuid,
        signer_key: &str,
        signature: &str,
        ip_address: Option<std::net::IpAddr>,
    ) -> Result<MintAuthSignature, MintAuthError> {
        let ip = ip_address.map(|ip| ipnetwork::IpNetwork::from(ip));

        let sig = sqlx::query_as!(
            MintAuthSignature,
            r#"
            INSERT INTO mint_authorization_signatures
                (auth_request_id, signer_id, signer_key, signature, ip_address)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, auth_request_id, signer_id, signer_key, signature, signed_at,
                      ip_address AS "ip_address: ipnetwork::IpNetwork"
            "#,
            auth_request_id,
            signer_id,
            signer_key,
            signature,
            ip as Option<ipnetwork::IpNetwork>,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(MintAuthError::from)?;

        // Atomically increment collected_signatures and check threshold
        sqlx::query!(
            r#"
            UPDATE mint_authorization_requests
            SET collected_signatures = collected_signatures + 1,
                status = CASE
                    WHEN collected_signatures + 1 >= required_signatures
                         AND status = 'pending_signatures'
                    THEN 'threshold_met'::mint_auth_status
                    ELSE status
                END
            WHERE id = $1
            "#,
            auth_request_id,
        )
        .execute(&self.pool)
        .await
        .map_err(MintAuthError::from)?;

        Ok(sig)
    }

    pub async fn list_signatures(
        &self,
        auth_request_id: Uuid,
    ) -> Result<Vec<MintAuthSignature>, MintAuthError> {
        sqlx::query_as!(
            MintAuthSignature,
            r#"
            SELECT id, auth_request_id, signer_id, signer_key, signature, signed_at,
                   ip_address AS "ip_address: ipnetwork::IpNetwork"
            FROM mint_authorization_signatures
            WHERE auth_request_id = $1
            ORDER BY signed_at ASC
            "#,
            auth_request_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(MintAuthError::from)
    }

    pub async fn signature_exists(
        &self,
        auth_request_id: Uuid,
        signer_id: Uuid,
    ) -> Result<bool, MintAuthError> {
        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM mint_authorization_signatures
             WHERE auth_request_id = $1 AND signer_id = $2",
            auth_request_id,
            signer_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(MintAuthError::from)?
        .unwrap_or(0);
        Ok(count > 0)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Reserve verification lookup
    // ─────────────────────────────────────────────────────────────────────────

    /// Fetch the reserve verification snapshot and return (fiat_reserves, in_transit, created_at).
    pub async fn get_reserve_verification(
        &self,
        id: Uuid,
    ) -> Result<Option<(BigDecimal, BigDecimal, DateTime<Utc>)>, MintAuthError> {
        let row = sqlx::query!(
            r#"
            SELECT fiat_reserves, in_transit, created_at
            FROM historical_verification
            WHERE id = $1 AND is_collateralised = true
            "#,
            id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(MintAuthError::from)?;

        Ok(row.map(|r| (r.fiat_reserves, r.in_transit, r.created_at)))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Signer lookup
    // ─────────────────────────────────────────────────────────────────────────

    /// Returns (signer_id) if the public key belongs to an active signer.
    pub async fn find_active_signer_by_key(
        &self,
        stellar_public_key: &str,
    ) -> Result<Option<Uuid>, MintAuthError> {
        let id = sqlx::query_scalar!(
            "SELECT id FROM mint_signers WHERE stellar_public_key = $1 AND status = 'active'",
            stellar_public_key,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(MintAuthError::from)?;
        Ok(id)
    }

    /// Returns the required_threshold from mint_quorum_config.
    pub async fn get_required_threshold(&self) -> Result<i16, MintAuthError> {
        let threshold: i16 = sqlx::query_scalar!(
            "SELECT required_threshold FROM mint_quorum_config ORDER BY updated_at DESC LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(MintAuthError::from)?
        .unwrap_or(2); // safe default
        Ok(threshold)
    }

    /// Returns all active signer emails for notifications.
    pub async fn list_active_signer_emails(&self) -> Result<Vec<String>, MintAuthError> {
        let emails = sqlx::query_scalar!(
            "SELECT contact_email FROM mint_signers WHERE status = 'active'"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(MintAuthError::from)?;
        Ok(emails)
    }
}
