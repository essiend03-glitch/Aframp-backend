//! High-Speed Webhook Engine with Exponential Backoff
//! Sends cryptographically signed webhooks to merchants

use crate::merchant_gateway::models::*;
use crate::merchant_gateway::repository::WebhookDeliveryRepository;
use crate::merchant_gateway::webhook_queue::{webhook_idempotency_key, worker_pool_size};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::sync::Semaphore;
use tokio::time::interval;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

// ============================================================================
// WEBHOOK ENGINE
// ============================================================================

pub struct WebhookEngine {
    webhook_repo: Arc<WebhookDeliveryRepository>,
    http_client: Client,
    max_retries: u32,
    timeout_secs: u64,
    delivery_worker_id: String,
}

impl WebhookEngine {
    pub fn new(pool: PgPool) -> Self {
        let timeout_secs = std::env::var("WEBHOOK_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let http_client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            webhook_repo: Arc::new(WebhookDeliveryRepository::new(pool)),
            http_client,
            max_retries: 5,
            timeout_secs,
            delivery_worker_id: format!("webhook-worker-{}", Uuid::new_v4()),
        }
    }

    /// Send webhook notification to merchant
    /// Returns immediately after queuing - actual delivery is async
    #[instrument(skip(self, merchant, payment_intent))]
    pub async fn send_webhook(
        &self,
        merchant: &Merchant,
        payment_intent: &MerchantPaymentIntent,
        event_type: &str,
    ) -> Result<Uuid, String> {
        // Determine webhook URL (payment intent override or merchant default)
        let webhook_url = payment_intent
            .callback_url
            .as_ref()
            .or(merchant.webhook_url.as_ref())
            .ok_or_else(|| "No webhook URL configured".to_string())?;

        // Build webhook payload
        let payload = WebhookPayload {
            event_type: event_type.to_string(),
            payment_intent_id: payment_intent.id,
            merchant_reference: payment_intent.merchant_reference.clone(),
            amount_cngn: payment_intent.amount_cngn,
            status: payment_intent.status.clone(),
            stellar_tx_hash: payment_intent.stellar_tx_hash.clone(),
            paid_at: payment_intent.paid_at,
            confirmed_at: payment_intent.confirmed_at,
            metadata: payment_intent.metadata.clone(),
            timestamp: Utc::now(),
        };

        let payload_json = serde_json::to_value(&payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;

        // Generate HMAC signature
        let signature = self.generate_signature(&merchant.webhook_secret, &payload_json)?;

        let idempotency_key = webhook_idempotency_key(merchant.id, payment_intent.id, event_type);

        // Queue webhook delivery. The worker pool handles actual network I/O,
        // so callers can return immediately after this durable write.
        let webhook_delivery = self
            .webhook_repo
            .create(
                payment_intent.id,
                merchant.id,
                webhook_url,
                event_type,
                payload_json,
                &signature,
                &idempotency_key,
            )
            .await
            .map_err(|e| format!("Failed to queue webhook: {}", e))?;

        info!(
            webhook_id = %webhook_delivery.id,
            payment_intent_id = %payment_intent.id,
            merchant_id = %merchant.id,
            event_type = %event_type,
            idempotency_key = %webhook_delivery.idempotency_key,
            "Webhook queued for delivery"
        );

        Ok(webhook_delivery.id)
    }

    /// Deliver a single webhook (called by worker or immediate delivery)
    #[instrument(skip(self))]
    async fn deliver_webhook(&self, webhook_id: Uuid) -> Result<(), String> {
        // Fetch webhook details
        let webhook = self
            .webhook_repo
            .find_by_id(webhook_id)
            .await
            .map_err(|e| format!("Failed to fetch webhook: {}", e))?
            .ok_or_else(|| "Webhook not found".to_string())?;

        if matches!(
            webhook.status,
            WebhookStatus::Delivered | WebhookStatus::DeadLettered | WebhookStatus::Abandoned
        ) {
            return Ok(()); // Already delivered
        }

        if webhook.retry_count >= self.max_retries as i32 {
            self.webhook_repo
                .record_delivery_failure(
                    &webhook,
                    None,
                    "Max retries exceeded before delivery attempt",
                    self.max_retries,
                )
                .await
                .map_err(|e| format!("Failed to dead-letter webhook: {}", e))?;
            return Err("Max retries exceeded".to_string());
        }

        if let Some(circuit) = self
            .webhook_repo
            .active_circuit_for_endpoint(webhook.merchant_id, &webhook.webhook_url)
            .await
            .map_err(|e| format!("Failed to read webhook circuit breaker: {}", e))?
        {
            if let Some(opened_until) = circuit.opened_until {
                self.webhook_repo
                    .pause_endpoint_retry_until(
                        webhook.merchant_id,
                        &webhook.webhook_url,
                        opened_until,
                    )
                    .await
                    .map_err(|e| format!("Failed to pause webhook delivery: {}", e))?;
                warn!(
                    webhook_id = %webhook_id,
                    merchant_id = %webhook.merchant_id,
                    opened_until = %opened_until,
                    "Webhook endpoint circuit is open; delivery paused"
                );
                return Ok(());
            }
        }

        // Prepare HTTP request
        let payload_str = serde_json::to_string(&webhook.payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;

        let response = self
            .http_client
            .post(&webhook.webhook_url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Signature", &webhook.signature)
            .header("X-Webhook-Event", &webhook.event_type)
            .header("X-Webhook-Id", webhook_id.to_string())
            .header("X-Webhook-Timestamp", Utc::now().to_rfc3339())
            .header("x-aframp-idempotency-key", &webhook.idempotency_key)
            .body(payload_str)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                let response_body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Failed to read response".to_string());

                if status.is_success() {
                    // Success - mark as delivered
                    self.webhook_repo
                        .mark_delivered(webhook_id, status.as_u16() as i32, Some(&response_body))
                        .await
                        .map_err(|e| format!("Failed to mark webhook delivered: {}", e))?;
                    self.webhook_repo
                        .record_circuit_success(webhook.merchant_id, &webhook.webhook_url)
                        .await
                        .map_err(|e| format!("Failed to close webhook circuit: {}", e))?;

                    info!(
                        webhook_id = %webhook_id,
                        http_status = status.as_u16(),
                        "Webhook delivered successfully"
                    );
                    Ok(())
                } else {
                    // HTTP error - schedule retry
                    let error_msg = format!("HTTP {}: {}", status.as_u16(), response_body);
                    self.webhook_repo
                        .record_delivery_failure(
                            &webhook,
                            Some(status.as_u16() as i32),
                            &error_msg,
                            self.max_retries,
                        )
                        .await
                        .map_err(|e| format!("Failed to mark webhook failed: {}", e))?;

                    warn!(
                        webhook_id = %webhook_id,
                        http_status = status.as_u16(),
                        retry_count = webhook.retry_count + 1,
                        "Webhook delivery failed, will retry"
                    );
                    Err(error_msg)
                }
            }
            Err(e) => {
                // Network error - schedule retry
                let error_msg = format!("Network error: {}", e);
                self.webhook_repo
                    .record_delivery_failure(&webhook, None, &error_msg, self.max_retries)
                    .await
                    .map_err(|e| format!("Failed to mark webhook failed: {}", e))?;

                warn!(
                    webhook_id = %webhook_id,
                    error = %e,
                    retry_count = webhook.retry_count + 1,
                    "Webhook delivery failed, will retry"
                );
                Err(error_msg)
            }
        }
    }

    /// Generate HMAC-SHA256 signature for webhook payload
    fn generate_signature(
        &self,
        secret: &str,
        payload: &serde_json::Value,
    ) -> Result<String, String> {
        let payload_str = serde_json::to_string(payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| format!("Invalid HMAC key: {}", e))?;
        mac.update(payload_str.as_bytes());

        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }

    /// Verify webhook signature (for merchant to use)
    pub fn verify_signature(
        secret: &str,
        payload: &serde_json::Value,
        signature: &str,
    ) -> Result<bool, String> {
        let payload_str = serde_json::to_string(payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| format!("Invalid HMAC key: {}", e))?;
        mac.update(payload_str.as_bytes());

        let expected_signature = hex::encode(mac.finalize().into_bytes());
        Ok(expected_signature == signature)
    }

    fn clone_for_delivery(&self) -> Self {
        Self {
            webhook_repo: self.webhook_repo.clone(),
            http_client: self.http_client.clone(),
            max_retries: self.max_retries,
            timeout_secs: self.timeout_secs,
            delivery_worker_id: self.delivery_worker_id.clone(),
        }
    }
}

// ============================================================================
// WEBHOOK RETRY WORKER
// ============================================================================

pub struct WebhookRetryWorker {
    webhook_repo: Arc<WebhookDeliveryRepository>,
    webhook_engine: Arc<WebhookEngine>,
    poll_interval_secs: u64,
    batch_size: i64,
    max_concurrency: usize,
}

impl WebhookRetryWorker {
    pub fn new(pool: PgPool, webhook_engine: Arc<WebhookEngine>) -> Self {
        let poll_interval_secs = std::env::var("WEBHOOK_RETRY_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let batch_size = std::env::var("WEBHOOK_RETRY_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);
        let max_concurrency = std::env::var("WEBHOOK_WORKER_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16);

        Self {
            webhook_repo: Arc::new(WebhookDeliveryRepository::new(pool)),
            webhook_engine,
            poll_interval_secs,
            batch_size,
            max_concurrency,
        }
    }

    pub async fn run(self, mut shutdown_rx: watch::Receiver<bool>) {
        info!(
            poll_interval_secs = self.poll_interval_secs,
            batch_size = self.batch_size,
            max_concurrency = self.max_concurrency,
            "Webhook retry worker started"
        );

        let mut ticker = interval(Duration::from_secs(self.poll_interval_secs));
        ticker.tick().await; // Skip first immediate tick

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Webhook retry worker: shutdown signal received");
                        break;
                    }
                }
                _ = ticker.tick() => {
                    if let Err(e) = self.process_pending_webhooks().await {
                        error!(error = %e, "Webhook retry cycle failed");
                    }
                }
            }
        }

        info!("Webhook retry worker stopped");
    }

    async fn process_pending_webhooks(&self) -> Result<(), String> {
        let pending = self
            .webhook_repo
            .find_pending_for_retry(self.batch_size)
            .await
            .map_err(|e| format!("Failed to fetch pending webhooks: {}", e))?;

        if pending.is_empty() {
            return Ok(());
        }

        let pool_size = worker_pool_size(pending.len(), self.max_concurrency);
        info!(
            count = pending.len(),
            pool_size, "Processing queued webhooks"
        );

        let permits = Arc::new(Semaphore::new(pool_size));
        let mut tasks = Vec::with_capacity(pending.len());

        for webhook in pending {
            let engine = self.webhook_engine.clone();
            let webhook_id = webhook.id;
            let permit = permits
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| format!("Webhook worker semaphore closed: {}", e))?;
            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) = engine.deliver_webhook(webhook_id).await {
                    warn!(webhook_id = %webhook_id, error = %e, "Webhook retry failed");
                }
            }));
        }

        for task in tasks {
            if let Err(e) = task.await {
                warn!(error = %e, "Webhook worker task join failed");
            }
        }

        Ok(())
    }
}
