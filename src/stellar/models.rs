/// Data models for Stellar submission engine
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::sync::atomic::{AtomicI64, AtomicU64};
use std::sync::Arc;

/// Submission channel account in the pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionChannel {
    pub id: Uuid,
    pub issuer_id: Uuid,
    pub environment: String,
    pub channel_account_id: String,
    pub channel_index: i32,
    pub current_sequence: i64,
    pub reserved_sequence: i64,
    pub balance_xlm: Decimal,
    pub min_balance_threshold: Decimal,
    pub is_active: bool,
    pub in_rotation: bool,
    pub total_submitted: i64,
    pub total_successful: i64,
    pub total_failed: i64,
    pub consecutive_failures: i32,
    pub last_error_code: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// In-memory representation of a channel with atomic sequence coordination
#[derive(Debug, Clone)]
pub struct ChannelHandle {
    pub db_id: Uuid,
    pub account_id: String,
    pub index: i32,
    pub sequence_counter: Arc<AtomicI64>,  // current_sequence
    pub reserved_counter: Arc<AtomicI64>,  // reserved_sequence
    pub submission_count: Arc<AtomicU64>,  // lifetime submissions
    pub success_count: Arc<AtomicU64>,     // lifetime successes
    pub failure_count: Arc<AtomicU64>,     // lifetime failures
    pub circuit_breaker_state: Arc<tokio::sync::Mutex<CircuitBreakerState>>,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerState {
    pub consecutive_failures: u32,
    pub threshold: u32,
    pub is_open: bool,
    pub last_failure_at: Option<DateTime<Utc>>,
}

/// Stellar transaction submission log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionLogEntry {
    pub id: Uuid,
    pub issuer_id: Uuid,
    pub channel_id: Uuid,
    pub submission_index: i64,
    pub tx_envelope_hash: String,
    pub tx_envelope_xdr: String,
    pub submission_fee_stroops: i64,
    pub surge_fee_percent: Decimal,
    pub submission_attempt: i32,
    pub submitted_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub stellar_ledger_hash: Option<String>,
    pub stellar_ledger_number: Option<i64>,
    pub stellar_tx_hash: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_reason: Option<String>,
    pub retry_count: i32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub final_status: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Fee statistics from Horizon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeStats {
    pub ledger_capacity_usage: String,
    pub last_ledger_base_fee: i64,
    pub last_ledger: i64,
    pub network_capacity_usage: String,
    pub percentile_10_accepted_fee: i64,
    pub percentile_20_accepted_fee: i64,
    pub percentile_30_accepted_fee: i64,
    pub percentile_40_accepted_fee: i64,
    pub percentile_50_accepted_fee: i64,
    pub percentile_60_accepted_fee: i64,
    pub percentile_70_accepted_fee: i64,
    pub percentile_80_accepted_fee: i64,
    pub percentile_90_accepted_fee: i64,
    pub percentile_95_accepted_fee: i64,
    pub percentile_99_accepted_fee: i64,
}

/// Dynamic fee configuration
#[derive(Debug, Clone)]
pub struct FeeConfiguration {
    pub base_fee: i64,           // stroops
    pub min_fee: i64,            // stroops
    pub max_fee: i64,            // stroops (cap)
    pub surge_threshold: f64,    // 0.8 = 80% ledger usage
    pub surge_multiplier: f64,   // 1.5 = 150% of recommended
    pub low_capacity_fee: i64,   // stroops during high usage
}

impl Default for FeeConfiguration {
    fn default() -> Self {
        Self {
            base_fee: 100,
            min_fee: 100,
            max_fee: 10_000,
            surge_threshold: 0.8,
            surge_multiplier: 1.5,
            low_capacity_fee: 1_000,
        }
    }
}

/// Submission metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionMetrics {
    pub timestamp: DateTime<Utc>,
    pub throughput_tps: f64,
    pub avg_submission_duration_ms: f64,
    pub current_surge_fee_stroops: i64,
    pub channel_exhaustion_percent: f64,
    pub total_channels_active: u32,
    pub total_channels_inactive: u32,
    pub pending_confirmations: u32,
    pub failed_submissions_24h: u64,
}

/// Horizon transaction response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorizonTransaction {
    pub id: String,
    pub paging_token: String,
    pub hash: String,
    pub ledger: i64,
    pub created_at: String,
    pub source_account: String,
    pub source_account_sequence: i64,
    pub fee_charged: i64,
    pub max_fee: i64,
    pub operation_count: i32,
    pub envelope_xdr: String,
    pub result_xdr: String,
    pub result_meta_xdr: String,
    pub successful: bool,
}

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_backoff_ms: 100,
            max_backoff_ms: 30_000,
            backoff_multiplier: 2.0,
        }
    }
}

// Re-export commonly used types
use sqlx::types::Decimal;

#[derive(Debug, Clone)]
pub struct ChannelExhaustionAlert {
    pub channel_id: Uuid,
    pub available_slots: i32,
    pub total_slots: i32,
    pub utilization_percent: Decimal,
}

#[derive(Debug, Clone)]
pub struct ConfirmationDelayAlert {
    pub tx_log_id: Uuid,
    pub submitted_at: DateTime<Utc>,
    pub ledgers_to_confirm: i32,
    pub confirmation_time_seconds: Decimal,
}
