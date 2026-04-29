//! Banking Integration — Inbound Webhook Handler (Issue #407)
//!
//! Receives and processes bank/provider webhook events:
//! - charge.success / transfer.success → mark transfer settled
//! - charge.failed / transfer.failed   → mark transfer failed
//! - Unknown events are stored as 'ignored' for audit purposes.

use super::repository::BankingRepository;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub struct BankWebhookProcessor {
    repo: Arc<BankingRepository>,
}

impl BankWebhookProcessor {
    pub fn new(repo: Arc<BankingRepository>) -> Self {
        Self { repo }
    }

    /// Process a raw inbound webhook payload.
    /// Always stores the raw event first (idempotent), then processes it.
    pub async fn process(
        &self,
        provider: &str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let event_type = payload
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let provider_event_id = payload
            .pointer("/data/id")
            .and_then(|v| v.as_str())
            .or_else(|| payload.pointer("/data/reference").and_then(|v| v.as_str()))
            .unwrap_or("unknown");

        // Idempotent store — duplicate events are silently ignored
        let event = self
            .repo
            .upsert_webhook_event(provider, event_type, provider_event_id, payload)
            .await?;

        // Already processed — skip
        if event.status == "processed" || event.status == "ignored" {
            info!(
                event_id = %event.id,
                status = %event.status,
                "Webhook already handled, skipping"
            );
            return Ok(());
        }

        let result = self.dispatch(event.id, event_type, payload).await;

        match result {
            Ok(()) => {
                self.repo
                    .mark_webhook_processed(event.id, None, None)
                    .await?;
            }
            Err(e) => {
                warn!(event_id = %event.id, error = %e, "Webhook processing failed");
                self.repo
                    .mark_webhook_failed(event.id, &e.to_string())
                    .await?;
            }
        }

        Ok(())
    }

    async fn dispatch(
        &self,
        event_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        match event_type {
            "charge.success" | "transfer.success" => {
                self.handle_success(event_id, payload).await
            }
            "charge.failed" | "transfer.failed" | "transfer.reversed" => {
                self.handle_failure(event_id, payload).await
            }
            other => {
                info!(event_type = %other, "Ignoring unhandled bank webhook event type");
                // Mark as ignored so it doesn't get retried
                sqlx::query(
                    "UPDATE bank_webhook_events SET status = 'ignored', processed_at = NOW() WHERE id = $1",
                )
                .bind(event_id)
                .execute(self.repo.pool())
                .await?;
                Ok(())
            }
        }
    }

    async fn handle_success(
        &self,
        event_id: Uuid,
        payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let reference = payload
            .pointer("/data/reference")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing reference in webhook payload"))?;

        let transfer = self
            .repo
            .get_transfer_by_idempotency_key(reference)
            .await?;

        if let Some(t) = transfer {
            self.repo
                .update_transfer_status(
                    t.id,
                    "success",
                    Some(reference),
                    Some(payload),
                    None,
                )
                .await?;
            self.repo
                .mark_webhook_processed(event_id, Some(t.linked_account_id), Some(t.id))
                .await?;
            info!(transfer_id = %t.id, "Transfer marked successful via webhook");
        } else {
            warn!(reference = %reference, "No matching transfer for webhook reference");
        }

        Ok(())
    }

    async fn handle_failure(
        &self,
        event_id: Uuid,
        payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let reference = payload
            .pointer("/data/reference")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing reference in webhook payload"))?;

        let reason = payload
            .pointer("/data/gateway_response")
            .and_then(|v| v.as_str())
            .unwrap_or("Provider declined");

        let transfer = self
            .repo
            .get_transfer_by_idempotency_key(reference)
            .await?;

        if let Some(t) = transfer {
            self.repo
                .update_transfer_status(t.id, "failed", Some(reference), Some(payload), Some(reason))
                .await?;
            self.repo
                .mark_webhook_processed(event_id, Some(t.linked_account_id), Some(t.id))
                .await?;
            warn!(transfer_id = %t.id, reason = %reason, "Transfer marked failed via webhook");
        }

        Ok(())
    }
}
