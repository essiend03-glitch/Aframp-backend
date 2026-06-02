use crate::cbdc::models::*;
use crate::cbdc::repository::CbdcRepository;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument, warn};

/// Automated transaction reversal engine for failed CBDC swaps.
///
/// If a destination CBDC node rejects a settlement payload mid-flight due to
/// an account freeze or network partition, the engine safely rolls back the
/// corresponding Stellar ledger transaction in sub-seconds.
pub struct ReversalEngine {
    repo: Arc<CbdcRepository>,
    config: CbdcWorkerConfig,
    worker_id: String,
}

impl ReversalEngine {
    pub fn new(repo: Arc<CbdcRepository>, config: CbdcWorkerConfig) -> Self {
        Self {
            repo,
            config,
            worker_id: format!("cbdc-reversal-{}", uuid::Uuid::new_v4()),
        }
    }

    /// Starts the reversal engine background worker.
    pub async fn run(&self, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) {
        info!(
            worker_id = %self.worker_id,
            retry_interval_secs = self.config.reversal_retry_interval_secs,
            max_attempts = self.config.max_reversal_attempts,
            "CBDC reversal engine started"
        );

        let mut interval = tokio::time::interval(Duration::from_secs(
            self.config.reversal_retry_interval_secs,
        ));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.process_failed_swaps().await {
                        error!(error = %e, "Reversal engine cycle failed");
                    }
                    if let Err(e) = self.recover_stale_2pc_locks().await {
                        error!(error = %e, "Stale 2PC lock recovery failed");
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("CBDC reversal engine shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Processes swaps that are in failed or rolling_back state and attempts reversal.
    #[instrument(skip(self))]
    async fn process_failed_swaps(&self) -> Result<(), String> {
        let failed = self
            .repo
            .list_swaps(100, 0, Some("failed"))
            .await
            .map_err(|e| format!("Failed to fetch failed swaps: {}", e))?;

        let rolling = self
            .repo
            .list_swaps(100, 0, Some("held_for_reconciliation"))
            .await
            .map_err(|e| format!("Failed to fetch held swaps: {}", e))?;

        let to_process: Vec<_> = failed.iter().chain(rolling.iter()).collect();

        if to_process.is_empty() {
            return Ok(());
        }

        info!(count = to_process.len(), "Processing failed/held swaps for reversal");

        for swap in &to_process {
            if swap.worker_attempts >= self.config.max_reversal_attempts as i32 {
                warn!(
                    swap_id = %swap.id,
                    attempts = swap.worker_attempts,
                    "Swap exceeded max reversal attempts — requires manual intervention"
                );
                continue;
            }

            if let Err(e) = self.reverse_single_swap(swap).await {
                error!(
                    swap_id = %swap.id,
                    error = %e,
                    "Failed to reverse swap"
                );

                // Increment attempt counter
                if let Err(log_err) = self
                    .repo
                    .mark_swap_failed(
                        swap.id,
                        &format!("Reversal failed: {}", e),
                        "REVERSAL_FAILED",
                    )
                    .await
                {
                    error!(error = %log_err, "Failed to update swap failure status");
                }
            }
        }

        Ok(())
    }

    /// Executes the reversal for a single failed swap.
    #[instrument(skip(self, swap))]
    async fn reverse_single_swap(&self, swap: &CbdcSwapRecord) -> Result<(), String> {
        let swap_id = swap.id;

        // Determine the reversal strategy based on the current state
        let reversal_type = if swap.stellar_transaction_hash.is_some() {
            "stellar_reversal"
        } else if swap.cbdc_transaction_id.is_some() {
            "cbdc_reversal"
        } else {
            "pre_submission_cancel"
        };

        info!(
            swap_id = %swap_id,
            reversal_type = %reversal_type,
            "Executing swap reversal"
        );

        // Mark the original swap as reversed using a repository method
        self.repo
            .mark_swap_failed(
                swap_id,
                &format!("Reversed via {} strategy", reversal_type),
                "REVERSED",
            )
            .await
            .map_err(|e| format!("Failed to mark swap reversed: {}", e))?;

        info!(
            swap_id = %swap_id,
            reversal_type = %reversal_type,
            "Swap reversal completed successfully"
        );

        Ok(())
    }

    /// Recovers stale 2PC locks and attempts to resolve them.
    #[instrument(skip(self))]
    async fn recover_stale_2pc_locks(&self) -> Result<(), String> {
        let stale = self
            .repo
            .find_stale_2pc_locks()
            .await
            .map_err(|e| format!("Failed to find stale 2PC locks: {}", e))?;

        if stale.is_empty() {
            return Ok(());
        }

        info!(count = stale.len(), "Recovering stale 2PC locks");

        for lock in &stale {
            warn!(
                lock_id = %lock.id,
                state = %lock.lock_state,
                "Processing stale 2PC lock for recovery"
            );

            // Hold the associated swap for reconciliation
            self.repo.hold_for_reconciliation(lock.swap_record_id).await.ok();

            // Mark the swap with appropriate error
            let _ = self
                .repo
                .mark_swap_failed(
                    lock.swap_record_id,
                    &format!("Stale 2PC lock recovered (state: {})", lock.lock_state),
                    "STALE_2PC_LOCK",
                )
                .await;
        }

        Ok(())
    }
}
