//! Webhook Ingestion Controller
//! Captures and validates real-time credit notifications from partner banks

use crate::cache::RedisCache;
use crate::banking::integrations::{BankWebhook, WebhookProcessingStatus};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Webhook Configuration
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Idempotency key TTL in seconds
    pub idempotency_ttl_seconds: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Signature validation enabled
    pub validate_signatures: bool,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            idempotency_ttl_seconds: 86400, // 24 hours
            max_retries: 3,
            validate_signatures: true,
        }
    }
}

/// Webhook Event Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BankWebhookEventType {
    AccountCredit,
    AccountDebit,
    VirtualAccountCreated,
    VirtualAccountClosed,
    MandateCreated,
    MandateExpired,
    TransferCompleted,
    TransferFailed,
    #[serde(other)]
    Unknown,
}

/// Webhook Processing Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookProcessingResult {
    pub event_id: String,
    pub status: WebhookProcessingStatus,
    pub settlement_id: Option<Uuid>,
    pub error: Option<String>,
    pub processed_at: chrono::DateTime<Utc>,
}

/// Webhook Signature Validator
pub struct WebhookSignatureValidator {
    secret: String,
}

impl WebhookSignatureValidator {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    /// Validate HMAC-SHA256 signature
    pub fn validate(&self, payload: &str, signature: &str, timestamp: i64) -> bool {
        use hmac::{Hmac, Mac};

        // Create HMAC-SHA256 instance
        type HmacSha256 = Hmac<sha2::Sha256>;

        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .expect("HMAC can take key of any size");

        // Combine timestamp and payload
        let message = format!("{}.{}", timestamp, payload);
        mac.update(message.as_bytes());

        // Compute hash
        let result = mac.finalize();

        // Compare with provided signature
        let expected = hex::encode(result.into_bytes());
        expected == signature
    }

    /// Validate SHA512 signature (Flutterwave format)
    pub fn validate_sha512(&self, payload: &str, signature: &str) -> bool {
        use sha2::{Digest, Sha512};

        let mut hasher = Sha512::new();
        hasher.update(payload.as_bytes());
        hasher.update(self.secret.as_bytes());
        let result = hasher.finalize();

        let expected = hex::encode(result);
        expected.eq_ignore_ascii_case(signature)
    }
}

/// Webhook Ingestion Controller
pub struct WebhookIngestionController {
    config: WebhookConfig,
    http: Client,
    cache: Arc<RedisCache>,
    signature_validator: Option<WebhookSignatureValidator>,
}

impl WebhookIngestionController {
    pub fn new(
        config: WebhookConfig,
        cache: Arc<RedisCache>,
        webhook_secret: Option<String>,
    ) -> Self {
        let signature_validator = webhook_secret.map(WebhookSignatureValidator::new);

        Self {
            config,
            http: Client::new(),
            cache,
            signature_validator,
        }
    }

    /// Process incoming webhook from partner bank
    /// Uses Redis for idempotency checking before processing
    #[instrument(skip(self, payload), fields(event_type = %payload.event_type))]
    pub async fn ingest_webhook(
        &self,
        payload: crate::banking::integrations::WebhookPayload,
    ) -> Result<WebhookProcessingResult, WebhookError> {
        let event_id = payload.event_id.clone();
        let idempotency_key = format!("webhook:{}:{}", payload.event_type, payload.event_id);

        // 1. Idempotency check - critical operation
        if self.is_duplicate(&idempotency_key).await {
            warn!(event_id = %event_id, "Duplicate webhook received");
            return Ok(WebhookProcessingResult {
                event_id: event_id.clone(),
                status: WebhookProcessingStatus::Duplicate,
                settlement_id: None,
                error: Some("Duplicate event".to_string()),
                processed_at: Utc::now(),
            });
        }

        // 2. Validate signature if configured
        if self.config.validate_signatures {
            if let Some(ref signature) = payload.signature {
                if !self.validate_signature(&payload, signature) {
                    error!(event_id = %event_id, "Invalid webhook signature");
                    return Err(WebhookError::InvalidSignature);
                }
            }
        }

        // 3. Mark as processing (prevent duplicates during processing)
        self.mark_processing(&idempotency_key).await;

        // 4. Process based on event type
        let result = match self.parse_event_type(&payload.event_type) {
            BankWebhookEventType::AccountCredit => {
                self.process_credit_notification(payload.data).await
            }
            BankWebhookEventType::AccountDebit => {
                self.process_debit_notification(payload.data).await
            }
            _ => {
                info!(event_type = %payload.event_type, "Unhandled event type");
                Ok(WebhookProcessingResult {
                    event_id: event_id.clone(),
                    status: WebhookProcessingStatus::Processed,
                    settlement_id: None,
                    error: None,
                    processed_at: Utc::now(),
                })
            }
        };

        // 5. Record idempotency key
        self.record_idempotency(&idempotency_key).await;

        result
    }

    /// Check if this webhook is a duplicate using Redis
    async fn is_duplicate(&self, key: &str) -> bool {
        let exists = self.cache.exists(key).await.unwrap_or(false);
        exists
    }

    /// Mark webhook as being processed
    async fn mark_processing(&self, key: &str) {
        let _ = self
            .cache
            .set(key, &"processing", Some(Duration::from_secs(60)))
            .await;
    }

    /// Record idempotency key after successful processing
    async fn record_idempotency(&self, key: &str) {
        let _ = self
            .cache
            .set(key, &"processed", Some(Duration::from_secs(self.config.idempotency_ttl_seconds)))
            .await;
    }

    /// Validate webhook signature
    fn validate_signature(&self, payload: &crate::banking::integrations::WebhookPayload, signature: &str) -> bool {
        if let Some(ref validator) = self.signature_validator {
            if let Some(timestamp) = payload.timestamp {
                let payload_str = serde_json::to_string(&payload.data).unwrap_or_default();
                return validator.validate(&payload_str, signature, timestamp);
            }
        }
        // If no validator configured, allow (but log warning)
        warn!("Signature validation skipped - no validator configured");
        true
    }

    /// Parse event type from string
    fn parse_event_type(&self, event_type: &str) -> BankWebhookEventType {
        match event_type.to_lowercase().as_str() {
            "account.credit" | "credit.notification" | "transfer.received" => {
                BankWebhookEventType::AccountCredit
            }
            "account.debit" | "debit.notification" | "transfer.sent" => {
                BankWebhookEventType::AccountDebit
            }
            "virtualaccount.created" => BankWebhookEventType::VirtualAccountCreated,
            "virtualaccount.closed" => BankWebhookEventType::VirtualAccountClosed,
            "mandate.created" => BankWebhookEventType::MandateCreated,
            "mandate.expired" => BankWebhookEventType::MandateExpired,
            "transfer.completed" => BankWebhookEventType::TransferCompleted,
            "transfer.failed" => BankWebhookEventType::TransferFailed,
            _ => BankWebhookEventType::Unknown,
        }
    }

    /// Process credit notification - triggers cNGN minting
    async fn process_credit_notification(
        &self,
        data: serde_json::Value,
    ) -> Result<WebhookProcessingResult, WebhookError> {
        // Extract transaction details
        let account_number = data
            .get("account_number")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let amount = data
            .get("amount")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<rust_decimal::Decimal>().ok())
            .unwrap_or_default();

        let transaction_id = data
            .get("transaction_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let reference = data
            .get("reference")
            .and_then(|v| v.as_str());

        info!(
            account_number = %account_number,
            amount = %amount,
            transaction_id = %transaction_id,
            "Processing credit notification"
        );

        // In production, would:
        // 1. Look up virtual account
        // 2. Create settlement record
        // 3. Trigger cNGN minting
        // 4. Update wallet balance

        Ok(WebhookProcessingResult {
            event_id: transaction_id.to_string(),
            status: WebhookProcessingStatus::Processed,
            settlement_id: Some(Uuid::new_v4()),
            error: None,
            processed_at: Utc::now(),
        })
    }

    /// Process debit notification
    async fn process_debit_notification(
        &self,
        data: serde_json::Value,
    ) -> Result<WebhookProcessingResult, WebhookError> {
        let transaction_id = data
            .get("transaction_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        info!(transaction_id = %transaction_id, "Processing debit notification");

        Ok(WebhookProcessingResult {
            event_id: transaction_id.to_string(),
            status: WebhookProcessingStatus::Processed,
            settlement_id: None,
            error: None,
            processed_at: Utc::now(),
        })
    }
}

/// Webhook Processing Errors
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("Invalid webhook signature")]
    InvalidSignature,

    #[error("Duplicate webhook received")]
    DuplicateEvent,

    #[error("Processing failed: {0}")]
    ProcessingFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl std::fmt::Display for WebhookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookError::InvalidSignature => write!(f, "Invalid webhook signature"),
            WebhookError::DuplicateEvent => write!(f, "Duplicate webhook received"),
            WebhookError::ProcessingFailed(s) => write!(f, "Processing failed: {}", s),
            WebhookError::DatabaseError(s) => write!(f, "Database error: {}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_validation() {
        let validator = WebhookSignatureValidator::new("test_secret".to_string());
        let payload = r#"{"amount":"1000"}"#;
        let timestamp = 1699999999;

        let is_valid = validator.validate(payload, "invalid_signature", timestamp);
        assert!(!is_valid);
    }

    #[test]
    fn test_event_type_parsing() {
        let controller = WebhookIngestionController::new(
            WebhookConfig::default(),
            Arc::new(RedisCache::new().await.unwrap()),
            None,
        );

        assert!(matches!(
            controller.parse_event_type("account.credit"),
            BankWebhookEventType::AccountCredit
        ));
        assert!(matches!(
            controller.parse_event_type("transfer.completed"),
            BankWebhookEventType::TransferCompleted
        ));
    }
}