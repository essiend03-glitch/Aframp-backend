//! #490 Gas & Fee Optimization — database repository.

use super::models::*;
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct FeeOptimizerRepository {
    pool: PgPool,
}

impl FeeOptimizerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_snapshot(&self, snap: &NetworkFeeSnapshot) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO network_fee_snapshots
               (snapshot_id, network, base_fee, priority_fee,
                ema_base_fee, ema_priority_fee, rpc_provider, block_reference)
               VALUES ($1,$2::chain_network,$3,$4,$5,$6,$7,$8)"#,
            snap.snapshot_id,
            snap.network.clone() as ChainNetwork,
            snap.base_fee,
            snap.priority_fee,
            snap.ema_base_fee,
            snap.ema_priority_fee,
            snap.rpc_provider,
            snap.block_reference,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_policy(
        &self,
        network: &ChainNetwork,
        urgency: &UrgencyWindow,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<FeeOptimizationPolicy>> {
        let row = sqlx::query_as!(
            FeeOptimizationPolicy,
            r#"SELECT policy_id, tenant_id,
                      network AS "network: ChainNetwork",
                      urgency AS "urgency: UrgencyWindow",
                      max_fee_cap, fee_multiplier, congestion_halt_threshold,
                      enabled, created_at, updated_at
               FROM fee_optimization_policies
               WHERE network = $1::chain_network
                 AND urgency = $2::urgency_window
                 AND (tenant_id = $3 OR tenant_id IS NULL)
                 AND enabled = TRUE
               ORDER BY tenant_id NULLS LAST
               LIMIT 1"#,
            network.clone() as ChainNetwork,
            urgency.clone() as UrgencyWindow,
            tenant_id,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn insert_gas_log(&self, log: &ExecutionGasLog) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO execution_gas_logs
               (gas_log_id, parent_tx_id, network, urgency,
                estimated_fee, tx_hash, nonce_or_sequence, status)
               VALUES ($1,$2,$3::chain_network,$4::urgency_window,$5,$6,$7,$8::gas_log_status)"#,
            log.gas_log_id,
            log.parent_tx_id,
            log.network.clone() as ChainNetwork,
            log.urgency.clone() as UrgencyWindow,
            log.estimated_fee,
            log.tx_hash,
            log.nonce_or_sequence,
            log.status.clone() as GasLogStatus,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn bump_gas_log(
        &self,
        gas_log_id: Uuid,
        new_fee: &sqlx::types::BigDecimal,
    ) -> Result<()> {
        sqlx::query!(
            r#"UPDATE execution_gas_logs
               SET bump_count = bump_count + 1,
                   estimated_fee = $2,
                   status = 'bumped',
                   last_bumped_at = NOW()
               WHERE gas_log_id = $1"#,
            gas_log_id,
            new_fee,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn confirm_gas_log(
        &self,
        gas_log_id: Uuid,
        actual_fee: &sqlx::types::BigDecimal,
    ) -> Result<()> {
        sqlx::query!(
            r#"UPDATE execution_gas_logs
               SET actual_fee = $2, status = 'confirmed', confirmed_at = NOW()
               WHERE gas_log_id = $1"#,
            gas_log_id,
            actual_fee,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List gas logs that are stalled (pending/submitted/bumped) past SLA.
    pub async fn list_stalled_logs(&self) -> Result<Vec<ExecutionGasLog>> {
        let rows = sqlx::query_as!(
            ExecutionGasLog,
            r#"SELECT gas_log_id, parent_tx_id,
                      network AS "network: ChainNetwork",
                      urgency AS "urgency: UrgencyWindow",
                      estimated_fee, actual_fee, bump_count,
                      tx_hash, nonce_or_sequence,
                      status AS "status: GasLogStatus",
                      submitted_at, confirmed_at, last_bumped_at
               FROM execution_gas_logs
               WHERE status IN ('pending','submitted','bumped')
                 AND submitted_at < NOW() - INTERVAL '3 minutes'"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
