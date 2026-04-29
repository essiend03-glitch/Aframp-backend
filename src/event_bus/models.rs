use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Well-known platform event types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    TransactionCreated,
    TransactionCompleted,
    TransactionFailed,
    AccountVerified,
    AccountSuspended,
    MintRequested,
    MintApproved,
    MintCompleted,
    BurnCompleted,
    PaymentInitiated,
    PaymentConfirmed,
    PaymentFailed,
    KycApproved,
    KycRejected,
    LendingPositionOpened,
    LendingPositionAtRisk,
    LendingPositionLiquidated,
    TravelRuleTriggered,
    TravelRuleAcknowledged,
    Custom(String),
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::Custom(s) => write!(f, "{}", s),
            _ => write!(f, "{:?}", self),
        }
    }
}

/// Delivery status of an event message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "event_delivery_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EventDeliveryStatus {
    Pending,
    Delivered,
    Failed,
    DeadLettered,
}

/// A platform event published to the event bus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub payload: Value,
    pub metadata: Value,
    pub published_at: DateTime<Utc>,
    pub schema_version: String,
}

impl PlatformEvent {
    pub fn new(
        event_type: EventType,
        aggregate_id: impl Into<String>,
        aggregate_type: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            aggregate_id: aggregate_id.into(),
            aggregate_type: aggregate_type.into(),
            payload,
            metadata: serde_json::json!({}),
            published_at: Utc::now(),
            schema_version: "1.0".to_string(),
        }
    }
}

/// Persisted event envelope for audit trail and replay
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EventRecord {
    pub event_id: Uuid,
    pub event_type: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub payload: Value,
    pub metadata: Value,
    pub schema_version: String,
    pub published_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Dead-letter queue entry for failed event processing
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeadLetterEntry {
    pub dlq_id: Uuid,
    pub event_id: Uuid,
    pub consumer_name: String,
    pub failure_reason: String,
    pub retry_count: i32,
    pub last_attempted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Consumer subscription registration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EventSubscription {
    pub subscription_id: Uuid,
    pub consumer_name: String,
    pub event_types: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Idempotency record — prevents duplicate processing
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessedEventRecord {
    pub event_id: Uuid,
    pub consumer_name: String,
    pub processed_at: DateTime<Utc>,
}
