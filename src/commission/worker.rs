//! Batch payout worker (Issue #471).
//!
//! Periodically aggregates unpaid partner commissions, creates Stellar payment
//! operations, and records them in `commission_payout_records`.
//!
//! A Redis distributed lock prevents concurrent execution across replicas.

use std::{sync::Arc, time::Duration, time::Instant};

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use super::{
    metrics,
    models::PayoutStatus,
    repository::CommissionRepository,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

pub struct PayoutWorkerConfig {
    pub interval: Duration,
    pub lock_ttl: Duration,
    pub min_payout_stroops: i64, // skip partners below this threshold
    pub system_user_id: Uuid,
}

impl Default for PayoutWorkerConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3600),   // hourly
            lock_ttl: Duration::from_secs(300),     // 5 min lock
            min_payout_stroops: 1_000_000,          // ≥ 0.1 cNGN
            system_user_id: Uuid::nil(),
        }
    }
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

pub struct PayoutWorker {
    repo: Arc<CommissionRepository>,
    redis: Pool<RedisConnectionManager>,
    config: PayoutWorkerConfig,
}

const LOCK_KEY: &str = "commission:payout:lock";

impl PayoutWorker {
    pub fn new(
        repo: Arc<CommissionRepository>,
        redis: Pool<RedisConnectionManager>,
        config: PayoutWorkerConfig,
    ) -> Self {
        Self { repo, redis, config }
    }

    /// Spawn the worker as a background Tokio task.
    pub fn spawn(worker: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(worker.config.interval).await;
                if let Err(e) = worker.run_once().await {
                    error!(error = %e, "payout worker cycle failed");
                }
            }
        });
    }

    /// Execute one full payout cycle under a Redis distributed lock.
    #[instrument(skip(self))]
    pub async fn run_once(&self) -> anyhow::Result<()> {
        let lock_token = Uuid::new_v4().to_string();

        if !self.acquire_lock(&lock_token).await? {
            info!("payout lock held by another instance, skipping");
            return Ok(());
        }

        let result = self.process_pending_payouts().await;

        // Always release the lock, even on error
        if let Err(e) = self.release_lock(&lock_token).await {
            warn!(error = %e, "failed to release payout lock");
        }

        result
    }

    async fn process_pending_payouts(&self) -> anyhow::Result<()> {
        let pool = self.repo.pool().clone();

        // Collect all partners with unpaid balance above threshold
        let partners: Vec<(Uuid, String, i64)> = sqlx::query_as(
            r#"
            SELECT p.id, p.payout_address, b.accrued_stroops - b.paid_stroops AS unpaid
            FROM partners p
            JOIN partner_commission_balances b ON b.partner_id = p.id
            LEFT JOIN partner_integration_settings s ON s.partner_id = p.id
            WHERE b.accrued_stroops - b.paid_stroops >= $1
              AND p.is_active = TRUE
            ORDER BY unpaid DESC
            "#,
        )
        .bind(self.config.min_payout_stroops)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        if partners.is_empty() {
            info!("no partners eligible for payout this cycle");
            return Ok(());
        }

        let batch_ref = chrono::Utc::now().format("batch-%Y-%m-%dT%H").to_string();

        for (partner_id, payout_address, unpaid_stroops) in partners {
            let start = Instant::now();
            self.payout_partner(partner_id, &payout_address, unpaid_stroops, &batch_ref)
                .await;
            metrics::observe_payout_duration(
                &partner_id.to_string(),
                start.elapsed().as_secs_f64(),
            );
        }

        Ok(())
    }

    /// Process a single partner payout within its own DB transaction.
    #[instrument(skip(self), fields(partner_id = %partner_id, unpaid_stroops))]
    async fn payout_partner(
        &self,
        partner_id: Uuid,
        payout_address: &str,
        unpaid_stroops: i64,
        batch_ref: &str,
    ) {
        let pool = self.repo.pool().clone();

        let result: anyhow::Result<()> = async {
            // Re-verify exact unpaid balance inside the transaction
            let unpaid = self.repo.unpaid_balance(partner_id).await?;
            if unpaid < self.config.min_payout_stroops {
                return Ok(());
            }

            let mut tx = pool.begin().await?;

            // 1. Create payout record (pending)
            let payout = self
                .repo
                .create_payout_record(
                    partner_id,
                    payout_address,
                    unpaid,
                    0,
                    batch_ref,
                    self.config.system_user_id,
                )
                .await?;

            // 2. Link all unpaid ledger entries to this payout
            let count = CommissionRepository::link_entries_to_payout_tx(
                &mut tx, partner_id, payout.id,
            )
            .await?;

            // 3. Mark balance as paid
            sqlx::query(
                "UPDATE partner_commission_balances SET paid_stroops = accrued_stroops, updated_at = NOW() WHERE partner_id = $1",
            )
            .bind(partner_id)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            // 4. Dispatch Stellar payment (mock — real impl hooks into stellar service)
            let stellar_hash = self
                .dispatch_stellar_payment(partner_id, payout_address, unpaid)
                .await;

            match stellar_hash {
                Ok(hash) => {
                    self.repo
                        .update_payout_status(payout.id, &PayoutStatus::Completed, Some(&hash), None)
                        .await?;
                    metrics::payout_dispatched(&partner_id.to_string(), "completed", unpaid);
                    info!(
                        payout_id = %payout.id,
                        partner_id = %partner_id,
                        total_stroops = unpaid,
                        entry_count = count,
                        stellar_tx_hash = %hash,
                        "payout completed"
                    );
                }
                Err(e) => {
                    self.repo
                        .update_payout_status(
                            payout.id,
                            &PayoutStatus::Failed,
                            None,
                            Some(&e.to_string()),
                        )
                        .await?;
                    metrics::payout_dispatched(&partner_id.to_string(), "failed", unpaid);
                    error!(payout_id = %payout.id, error = %e, "stellar payment failed");
                }
            }

            Ok(())
        }
        .await;

        if let Err(e) = result {
            error!(partner_id = %partner_id, error = %e, "payout processing error");
        }
    }

    /// Stub: dispatch cNGN on Stellar; replace with real StellarClient call.
    async fn dispatch_stellar_payment(
        &self,
        partner_id: Uuid,
        address: &str,
        stroops: i64,
    ) -> anyhow::Result<String> {
        // TODO: call crate::chains::stellar::StellarClient::send_payment(address, stroops)
        info!(
            partner_id = %partner_id,
            payout_address = %address,
            stroops,
            "stellar payment dispatched (stub)"
        );
        // Return a deterministic fake hash for now
        Ok(format!(
            "STUB_{}_{}",
            &partner_id.to_string()[..8],
            stroops
        ))
    }

    // -----------------------------------------------------------------------
    // Redis distributed lock (SET NX PX)
    // -----------------------------------------------------------------------

    async fn acquire_lock(&self, token: &str) -> anyhow::Result<bool> {
        let mut conn = self.redis.get().await?;
        let acquired: bool = redis::cmd("SET")
            .arg(LOCK_KEY)
            .arg(token)
            .arg("NX")
            .arg("PX")
            .arg(self.config.lock_ttl.as_millis() as u64)
            .query_async(&mut *conn)
            .await
            .unwrap_or(false);
        Ok(acquired)
    }

    async fn release_lock(&self, token: &str) -> anyhow::Result<()> {
        // Lua script for atomic check-and-delete
        let script = r#"
            if redis.call('GET', KEYS[1]) == ARGV[1] then
                return redis.call('DEL', KEYS[1])
            else
                return 0
            end
        "#;
        let mut conn = self.redis.get().await?;
        let _: i32 = redis::Script::new(script)
            .key(LOCK_KEY)
            .arg(token)
            .invoke_async(&mut *conn)
            .await?;
        Ok(())
    }
}
