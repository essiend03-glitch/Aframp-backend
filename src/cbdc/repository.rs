use crate::cbdc::models::*;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CbdcRepository {
    pool: PgPool,
}

impl CbdcRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Gateway CRUD ───────────────────────────────────────────────────────────

    #[instrument(skip(self))]
    pub async fn register_gateway(&self, req: &RegisterGatewayRequest) -> Result<CbdcGateway, sqlx::Error> {
        sqlx::query_as::<_, CbdcGateway>(
            r#"
            INSERT INTO cbdc_gateways (name, description, dlt_system, network_type, rpc_endpoint,
                ws_endpoint, chain_id, mtls_ca_cert_pem, mtls_client_cert_pem, node_identity,
                connection_timeout_ms, max_retries, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.dlt_system.as_str())
        .bind(req.network_type.as_deref().unwrap_or("sandbox"))
        .bind(&req.rpc_endpoint)
        .bind(&req.ws_endpoint)
        .bind(req.chain_id)
        .bind(&req.mtls_ca_cert_pem)
        .bind(&req.mtls_client_cert_pem)
        .bind(&req.node_identity)
        .bind(req.connection_timeout_ms.unwrap_or(5000))
        .bind(req.max_retries.unwrap_or(3))
        .bind(req.metadata.as_ref().unwrap_or(&serde_json::Value::Object(Default::default())))
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn list_gateways(&self) -> Result<Vec<CbdcGateway>, sqlx::Error> {
        sqlx::query_as::<_, CbdcGateway>(
            "SELECT * FROM cbdc_gateways ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn get_gateway(&self, id: Uuid) -> Result<Option<CbdcGateway>, sqlx::Error> {
        sqlx::query_as::<_, CbdcGateway>(
            "SELECT * FROM cbdc_gateways WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn update_gateway_status(
        &self,
        id: Uuid,
        status: &str,
    ) -> Result<CbdcGateway, sqlx::Error> {
        sqlx::query_as::<_, CbdcGateway>(
            r#"
            UPDATE cbdc_gateways
            SET health_status = $2, last_health_check_at = NOW(),
                last_healthy_at = CASE WHEN $2 = 'healthy' THEN NOW() ELSE last_healthy_at END
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(status)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn get_active_gateways(&self) -> Result<Vec<CbdcGateway>, sqlx::Error> {
        sqlx::query_as::<_, CbdcGateway>(
            "SELECT * FROM cbdc_gateways WHERE is_active = TRUE ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
    }

    // ── Swap Records ──────────────────────────────────────────────────────────

    #[instrument(skip(self))]
    pub async fn create_swap_record(&self, req: &InitiateSwapRequest) -> Result<CbdcSwapRecord, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            r#"
            INSERT INTO cbdc_swap_records (swap_type, status, stellar_asset_code, stellar_asset_issuer,
                stellar_amount, stellar_destination_account, cbdc_gateway_id, cbdc_recipient,
                cbdc_currency, cbdc_amount, idempotency_key, compliance_metadata, required_approvals)
            VALUES ($1, 'pending', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING *
            "#,
        )
        .bind(req.swap_type.as_str())
        .bind(&req.stellar_asset_code)
        .bind(&req.stellar_asset_issuer)
        .bind(&req.stellar_amount)
        .bind(&req.stellar_destination_account)
        .bind(req.cbdc_gateway_id)
        .bind(&req.cbdc_recipient)
        .bind(&req.cbdc_currency)
        .bind(&req.cbdc_amount)
        .bind(&req.idempotency_key)
        .bind(req.compliance_metadata.as_ref().unwrap_or(&serde_json::Value::Object(Default::default())))
        .bind(req.required_approvals.unwrap_or(1))
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn get_swap_record(&self, id: Uuid) -> Result<Option<CbdcSwapRecord>, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            "SELECT * FROM cbdc_swap_records WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn get_swap_by_idempotency(&self, key: &str) -> Result<Option<CbdcSwapRecord>, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            "SELECT * FROM cbdc_swap_records WHERE idempotency_key = $1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn list_pending_swaps(&self, limit: usize) -> Result<Vec<CbdcSwapRecord>, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            r#"
            SELECT * FROM cbdc_swap_records
            WHERE status IN ('pending', 'prepared')
            ORDER BY created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn update_swap_stellar_leg(
        &self,
        id: Uuid,
        tx_hash: &str,
        source: &str,
        sequence: i64,
    ) -> Result<CbdcSwapRecord, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            r#"
            UPDATE cbdc_swap_records
            SET stellar_transaction_hash = $2, stellar_source_account = $3,
                stellar_sequence_number = $4, status = 'committed_stellar',
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(tx_hash)
        .bind(source)
        .bind(sequence)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn update_swap_cbdc_leg(
        &self,
        id: Uuid,
        cbdc_tx_id: &str,
        block_id: &str,
        block_number: i64,
        confirmations: i32,
        status: &str,
    ) -> Result<CbdcSwapRecord, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            r#"
            UPDATE cbdc_swap_records
            SET cbdc_transaction_id = $2, cbdc_block_id = $3,
                cbdc_block_number = $4, cbdc_confirmations = $5,
                status = $6, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(cbdc_tx_id)
        .bind(block_id)
        .bind(block_number)
        .bind(confirmations)
        .bind(status)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn mark_swap_completed(&self, id: Uuid) -> Result<CbdcSwapRecord, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            r#"
            UPDATE cbdc_swap_records
            SET status = 'completed', two_phase_state = 'committed',
                worker_completed_at = NOW(), updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn mark_swap_failed(
        &self,
        id: Uuid,
        error_message: &str,
        error_code: &str,
    ) -> Result<CbdcSwapRecord, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            r#"
            UPDATE cbdc_swap_records
            SET status = 'failed', error_message = $2, error_code = $3,
                two_phase_state = CASE WHEN two_phase_state IN ('preparing', 'prepared')
                    THEN 'rolling_back' ELSE two_phase_state END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(error_message)
        .bind(error_code)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn hold_for_reconciliation(&self, id: Uuid) -> Result<CbdcSwapRecord, sqlx::Error> {
        sqlx::query_as::<_, CbdcSwapRecord>(
            r#"
            UPDATE cbdc_swap_records
            SET status = 'held_for_reconciliation', two_phase_state = 'rolling_back',
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    // ── 2PC Lock Operations ───────────────────────────────────────────────────

    #[instrument(skip(self))]
    pub async fn create_2pc_lock(
        &self,
        lock_key: &str,
        swap_record_id: Uuid,
        gateway_id: Option<Uuid>,
        lock_holder: &str,
        ttl_secs: u64,
    ) -> Result<TwoPcLock, sqlx::Error> {
        sqlx::query_as::<_, TwoPcLock>(
            r#"
            INSERT INTO cbdc_2pc_locks (lock_key, swap_record_id, gateway_id, lock_state,
                lock_holder, lock_expires_at, prepared_payload)
            VALUES ($1, $2, $3, 'preparing', $4, NOW() + ($5 || ' seconds')::INTERVAL, '{}'::jsonb)
            RETURNING *
            "#,
        )
        .bind(lock_key)
        .bind(swap_record_id)
        .bind(gateway_id)
        .bind(lock_holder)
        .bind(ttl_secs.to_string())
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn update_2pc_prepared(
        &self,
        id: Uuid,
        prepared_payload: &serde_json::Value,
    ) -> Result<TwoPcLock, sqlx::Error> {
        sqlx::query_as::<_, TwoPcLock>(
            r#"
            UPDATE cbdc_2pc_locks
            SET lock_state = 'prepared', prepared_payload = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(prepared_payload)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn update_2pc_committing(
        &self,
        id: Uuid,
        commit_payload: &serde_json::Value,
    ) -> Result<TwoPcLock, sqlx::Error> {
        sqlx::query_as::<_, TwoPcLock>(
            r#"
            UPDATE cbdc_2pc_locks
            SET lock_state = 'committing', commit_payload = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(commit_payload)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn update_2pc_committed(&self, id: Uuid) -> Result<TwoPcLock, sqlx::Error> {
        sqlx::query_as::<_, TwoPcLock>(
            r#"
            UPDATE cbdc_2pc_locks
            SET lock_state = 'committed', updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn update_2pc_rolling_back(
        &self,
        id: Uuid,
        rollback_payload: &serde_json::Value,
    ) -> Result<TwoPcLock, sqlx::Error> {
        sqlx::query_as::<_, TwoPcLock>(
            r#"
            UPDATE cbdc_2pc_locks
            SET lock_state = 'rolling_back', rollback_payload = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(rollback_payload)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn update_2pc_rolled_back(&self, id: Uuid) -> Result<TwoPcLock, sqlx::Error> {
        sqlx::query_as::<_, TwoPcLock>(
            r#"
            UPDATE cbdc_2pc_locks
            SET lock_state = 'rolled_back', updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn find_stale_2pc_locks(&self) -> Result<Vec<TwoPcLock>, sqlx::Error> {
        sqlx::query_as::<_, TwoPcLock>(
            r#"
            SELECT * FROM cbdc_2pc_locks
            WHERE lock_state IN ('preparing', 'prepared', 'committing', 'rolling_back')
                AND lock_expires_at < NOW()
            ORDER BY lock_expires_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn heartbeat_2pc_lock(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE cbdc_2pc_locks
            SET last_heartbeat_at = NOW(), updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Signatory Vault ──────────────────────────────────────────────────────

    #[instrument(skip(self))]
    pub async fn add_signatory(&self, signatory: &CryptographicSignatory) -> Result<CryptographicSignatory, sqlx::Error> {
        sqlx::query_as::<_, CryptographicSignatory>(
            r#"
            INSERT INTO cryptographic_signatory_vault (swap_record_id, signatory_type, signatory_identity,
                signing_key_id, signing_algorithm, approval_action, approval_order, is_required,
                data_residency_region, expiry_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(signatory.swap_record_id)
        .bind(&signatory.signatory_type)
        .bind(&signatory.signatory_identity)
        .bind(&signatory.signing_key_id)
        .bind(&signatory.signing_algorithm)
        .bind(&signatory.approval_action)
        .bind(signatory.approval_order)
        .bind(signatory.is_required)
        .bind(&signatory.data_residency_region)
        .bind(signatory.expiry_at)
        .fetch_one(&self.pool)
        .await
    }

    #[instrument(skip(self))]
    pub async fn get_signatories_for_swap(
        &self,
        swap_record_id: Uuid,
    ) -> Result<Vec<CryptographicSignatory>, sqlx::Error> {
        sqlx::query_as::<_, CryptographicSignatory>(
            "SELECT * FROM cryptographic_signatory_vault WHERE swap_record_id = $1 ORDER BY approval_order",
        )
        .bind(swap_record_id)
        .fetch_all(&self.pool)
        .await
    }

    // ── Swap history ──────────────────────────────────────────────────────────

    #[instrument(skip(self))]
    pub async fn list_swaps(
        &self,
        limit: i64,
        offset: i64,
        status_filter: Option<&str>,
    ) -> Result<Vec<CbdcSwapRecord>, sqlx::Error> {
        match status_filter {
            Some(status) => sqlx::query_as::<_, CbdcSwapRecord>(
                "SELECT * FROM cbdc_swap_records WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            )
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await,
            None => sqlx::query_as::<_, CbdcSwapRecord>(
                "SELECT * FROM cbdc_swap_records ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await,
        }
    }
}
