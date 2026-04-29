//! Queue policy helpers for asynchronous merchant webhook delivery.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use uuid::Uuid;

pub const WEBHOOK_RETRY_BACKOFF_SECS: [i64; 5] = [60, 300, 900, 3_600, 14_400];
pub const DEFAULT_CIRCUIT_FAILURE_THRESHOLD: i32 = 5;
pub const DEFAULT_CIRCUIT_COOLDOWN_SECS: i64 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitDecision {
    Closed,
    OpenUntil(DateTime<Utc>),
}

pub fn webhook_idempotency_key(merchant_id: Uuid, subject_id: Uuid, event_type: &str) -> String {
    format!(
        "merchant-webhook:{}:{}:{}",
        merchant_id,
        event_type.trim().to_ascii_lowercase(),
        subject_id
    )
}

pub fn backoff_for_retry_attempt(attempt: i32) -> ChronoDuration {
    let index = attempt.saturating_sub(1) as usize;
    let secs = WEBHOOK_RETRY_BACKOFF_SECS
        .get(index)
        .copied()
        .unwrap_or(*WEBHOOK_RETRY_BACKOFF_SECS.last().unwrap());

    ChronoDuration::seconds(secs)
}

pub fn next_retry_at(now: DateTime<Utc>, next_attempt: i32) -> DateTime<Utc> {
    now + backoff_for_retry_attempt(next_attempt)
}

pub fn should_dead_letter(next_attempt: i32, max_retries: u32) -> bool {
    next_attempt >= max_retries as i32
}

pub fn is_circuit_breaker_failure(http_status: Option<i32>) -> bool {
    matches!(http_status, Some(status) if status >= 500)
}

pub fn circuit_decision_after_failure(
    now: DateTime<Utc>,
    consecutive_failures: i32,
    http_status: Option<i32>,
    failure_threshold: i32,
    cooldown_secs: i64,
) -> CircuitDecision {
    if is_circuit_breaker_failure(http_status) && consecutive_failures >= failure_threshold {
        CircuitDecision::OpenUntil(now + ChronoDuration::seconds(cooldown_secs))
    } else {
        CircuitDecision::Closed
    }
}

pub fn worker_pool_size(queue_depth: usize, configured_max: usize) -> usize {
    queue_depth.clamp(1, configured_max.max(1))
}
