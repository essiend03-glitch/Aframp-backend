//! #491 Compliance Oracle — database repository.

use super::models::*;
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct ComplianceOracleRepository {
    pool: PgPool,
}

impl ComplianceOracleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_active_oracles(&self) -> Result<Vec<ComplianceOracle>> {
        let rows = sqlx::query_as!(
            ComplianceOracle,
            r#"SELECT oracle_id, name, endpoint_url,
                      status AS "status: OracleStatus",
                      public_key, price_per_query_usd_cents, sla_ms, priority,
                      last_checked_at, created_at, updated_at
               FROM compliance_oracles
               WHERE status = 'active'
               ORDER BY priority ASC"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_valid_attestation(
        &self,
        address: &str,
    ) -> Result<Option<IdentityAttestation>> {
        let row = sqlx::query_as!(
            IdentityAttestation,
            r#"SELECT attestation_id, cross_chain_address, did_identifier,
                      did_method AS "did_method: DidMethod",
                      oracle_id, proof_hash, zk_proof_ref, issuer_signature,
                      status AS "status: AttestationStatus",
                      is_sanctions_clear, is_kyc_verified, is_aml_clear,
                      issued_at, expires_at, revoked_at, created_at
               FROM identity_attestations
               WHERE cross_chain_address = $1
                 AND status = 'valid'
                 AND expires_at > NOW()
               ORDER BY expires_at DESC
               LIMIT 1"#,
            address
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn insert_attestation(&self, att: &IdentityAttestation) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO identity_attestations
               (attestation_id, cross_chain_address, did_identifier,
                did_method, oracle_id, proof_hash, zk_proof_ref,
                issuer_signature, status, is_sanctions_clear,
                is_kyc_verified, is_aml_clear, issued_at, expires_at)
               VALUES ($1,$2,$3,$4::did_method,$5,$6,$7,$8,$9::attestation_status,
                       $10,$11,$12,$13,$14)"#,
            att.attestation_id,
            att.cross_chain_address,
            att.did_identifier,
            att.did_method.clone() as DidMethod,
            att.oracle_id,
            att.proof_hash,
            att.zk_proof_ref,
            att.issuer_signature,
            att.status.clone() as AttestationStatus,
            att.is_sanctions_clear,
            att.is_kyc_verified,
            att.is_aml_clear,
            att.issued_at,
            att.expires_at,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_query_log(&self, log: &ComplianceQueryLog) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO compliance_query_logs
               (query_id, originating_tx_id, cross_chain_address, oracle_id,
                outcome, latency_ms, cache_hit, signature_envelope, error_detail)
               VALUES ($1,$2,$3,$4,$5::query_outcome,$6,$7,$8,$9)"#,
            log.query_id,
            log.originating_tx_id,
            log.cross_chain_address,
            log.oracle_id,
            log.outcome.clone() as QueryOutcome,
            log.latency_ms,
            log.cache_hit,
            log.signature_envelope,
            log.error_detail,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_oracle_heartbeat(&self, oracle_id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE compliance_oracles SET last_checked_at = NOW(), updated_at = NOW()
             WHERE oracle_id = $1",
            oracle_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
