use crate::cbdc::gateway::{DltGatewayClient, GatewayConnectionStatus};
use crate::cbdc::models::*;
use crate::cbdc::repository::CbdcRepository;
use crate::cbdc::two_pc::TwoPhaseCommitManager;
use crate::cbdc::validator::{ScreeningResult, SwapValidator};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};

/// Automated settlement worker that monitors the CBDC settlement event stream
/// and fires matching trustline payment or token mint sequences on Stellar.
pub struct SettlementWorker {
    repo: Arc<CbdcRepository>,
    two_pc: Arc<TwoPhaseCommitManager>,
    validator: Arc<SwapValidator>,
    gateway_pool: Arc<RwLock<Vec<Arc<DltGatewayClient>>>>,
    config: CbdcWorkerConfig,
    worker_id: String,
    running: Arc<RwLock<bool>>,
}

impl SettlementWorker {
    pub fn new(
        repo: Arc<CbdcRepository>,
        two_pc: Arc<TwoPhaseCommitManager>,
        validator: Arc<SwapValidator>,
        gateway_pool: Arc<RwLock<Vec<Arc<DltGatewayClient>>>>,
        config: CbdcWorkerConfig,
    ) -> Self {
        Self {
            repo,
            two_pc,
            validator,
            gateway_pool,
            config,
            worker_id: format!("cbdc-settlement-{}", uuid::Uuid::new_v4()),
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// Starts the settlement worker loop. Runs until the shutdown signal is received.
    pub async fn run(&self, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) {
        *self.running.write().await = true;
        info!(
            worker_id = %self.worker_id,
            poll_interval_secs = self.config.settlement_poll_interval_secs,
            "CBDC settlement worker started"
        );

        let mut interval = tokio::time::interval(Duration::from_secs(
            self.config.settlement_poll_interval_secs,
        ));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.process_pending_swaps().await {
                        error!(error = %e, "Settlement worker cycle failed");
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("CBDC settlement worker shutting down");
                        *self.running.write().await = false;
                        break;
                    }
                }
            }
        }
    }

    /// Processes all pending swaps in order of creation.
    #[instrument(skip(self))]
    async fn process_pending_swaps(&self) -> Result<(), String> {
        let pending = self
            .repo
            .list_pending_swaps(self.config.settlement_batch_size)
            .await
            .map_err(|e| format!("Failed to fetch pending swaps: {}", e))?;

        if pending.is_empty() {
            return Ok(());
        }

        info!(count = pending.len(), "Processing pending CBDC swaps");

        for swap in &pending {
            if let Err(e) = self.process_single_swap(swap).await {
                error!(
                    swap_id = %swap.id,
                    error = %e,
                    "Failed to process CBDC swap"
                );
            }
        }

        Ok(())
    }

    /// Processes a single swap through the complete settlement pipeline.
    #[instrument(skip(self, swap))]
    async fn process_single_swap(&self, swap: &CbdcSwapRecord) -> Result<(), String> {
        let swap_id = swap.id;

        // 1. Validate the swap payload against AML/compliance rules
        let validation_payload = serde_json::json!({
            "amount": swap.cbdc_amount.to_string(),
            "sender": swap.stellar_source_account,
            "recipient": swap.cbdc_recipient,
            "jurisdiction": swap.compliance_metadata.get("jurisdiction"),
            "compliance_metadata": swap.compliance_metadata,
        });

        let report = self.validator.validate(&validation_payload).await;
        if !report.is_valid {
            self.repo
                .mark_swap_failed(
                    swap_id,
                    &format!("Validation failed: {:?}", report.violations),
                    "VALIDATION_FAILED",
                )
                .await
                .map_err(|e| format!("Failed to mark swap failed: {}", e))?;
            return Err(format!("Swap validation failed: {:?}", report.violations));
        }

        if report.screening_result == ScreeningResult::Fail {
            self.repo
                .mark_swap_failed(swap_id, "AML screening failed", "AML_FAILED")
                .await
                .map_err(|e| format!("Failed to mark swap failed: {}", e))?;
            return Err("Swap rejected by AML screening".to_string());
        }

        // 2. Acquire 2PC lock
        let lock_key = format!("swap:{}", swap.idempotency_key);
        let lock = match self
            .two_pc
            .acquire_lock(swap_id, swap.cbdc_gateway_id, &lock_key)
            .await
        {
            Ok(l) => l,
            Err(e) => {
                warn!(swap_id = %swap_id, error = %e, "Could not acquire 2PC lock, will retry");
                return Ok(());
            }
        };

        // 3. Find gateway client
        let gateway = {
            let pool = self.gateway_pool.read().await;
            pool.iter()
                .find(|g| g.gateway_id() == swap.cbdc_gateway_id.unwrap_or_default())
                .cloned()
        };

        let gateway = match gateway {
            Some(g) => g,
            None => {
                self.two_pc
                    .rollback(&lock, &serde_json::json!({"reason": "gateway_not_found"}))
                    .await?;
                return Err("Gateway not found for swap".to_string());
            }
        };

        // 4. Prepare phase — submit transaction to CBDC DLT network
        let prepared_payload = serde_json::json!({
            "swap_id": swap_id.to_string(),
            "cbdc_recipient": swap.cbdc_recipient,
            "cbdc_amount": swap.cbdc_amount.to_string(),
            "cbdc_currency": swap.cbdc_currency,
            "source_account": swap.stellar_source_account,
        });

        let two_pc_lock = self.two_pc.prepare(&lock, &prepared_payload).await?;

        // 5. Submit transaction to CBDC gateway
        let tx_payload = serde_json::to_vec(&prepared_payload).unwrap_or_default();
        match gateway.submit_transaction(&tx_payload).await {
            Ok(cbdc_tx_hash) => {
                // 6. Wait for confirmations
                match gateway
                    .wait_for_confirmations(&cbdc_tx_hash, 2, 1000, 120)
                    .await
                {
                    Ok((block_id, block_number, confirmations)) => {
                        // Update swap with CBDC leg details
                        self.repo
                            .update_swap_cbdc_leg(
                                swap_id,
                                &cbdc_tx_hash,
                                &block_id,
                                block_number,
                                confirmations,
                                "committed_cbdc",
                            )
                            .await
                            .map_err(|e| format!("Failed to update CBDC leg: {}", e))?;

                        // 7. Commit the 2PC transaction
                        let commit_payload = serde_json::json!({
                            "cbdc_tx_hash": cbdc_tx_hash,
                            "block_id": block_id,
                            "block_number": block_number,
                            "confirmations": confirmations,
                            "completed_at": chrono::Utc::now().to_rfc3339(),
                        });
                        self.two_pc.commit(&two_pc_lock, &commit_payload).await?;

                        // 8. Mark swap as completed
                        self.repo.mark_swap_completed(swap_id).await.map_err(|e| {
                            format!("Failed to mark swap completed: {}", e)
                        })?;

                        info!(
                            swap_id = %swap_id,
                            cbdc_tx_hash = %cbdc_tx_hash,
                            block_number = block_number,
                            confirmations = confirmations,
                            "CBDC swap completed successfully"
                        );
                    }
                    Err(e) => {
                        // Confirmation timeout — roll back
                        self.two_pc
                            .rollback(&two_pc_lock, &serde_json::json!({"reason": e, "phase": "confirmation"}))
                            .await?;
                        return Err(format!("Confirmation failed: {}", e));
                    }
                }
            }
            Err(e) => {
                // Transaction submission failed — roll back
                self.two_pc
                    .rollback(&two_pc_lock, &serde_json::json!({"reason": e, "phase": "submission"}))
                    .await?;
                return Err(format!("Transaction submission failed: {}", e));
            }
        }

        Ok(())
    }
}
