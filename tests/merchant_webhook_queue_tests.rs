use chrono::Utc;
use uuid::Uuid;

use Bitmesh_backend::merchant_gateway::webhook_queue::{
    backoff_for_retry_attempt, circuit_decision_after_failure, is_circuit_breaker_failure,
    next_retry_at, should_dead_letter, webhook_idempotency_key, worker_pool_size, CircuitDecision,
};

#[test]
fn retry_backoff_uses_required_schedule() {
    assert_eq!(backoff_for_retry_attempt(1).num_seconds(), 60);
    assert_eq!(backoff_for_retry_attempt(2).num_seconds(), 300);
    assert_eq!(backoff_for_retry_attempt(3).num_seconds(), 900);
    assert_eq!(backoff_for_retry_attempt(4).num_seconds(), 3_600);
    assert_eq!(backoff_for_retry_attempt(5).num_seconds(), 14_400);
    assert_eq!(backoff_for_retry_attempt(99).num_seconds(), 14_400);
}

#[test]
fn retry_dead_letters_after_max_attempts() {
    assert!(!should_dead_letter(4, 5));
    assert!(should_dead_letter(5, 5));
    assert!(should_dead_letter(6, 5));
}

#[test]
fn idempotency_key_is_stable_for_same_event() {
    let merchant_id = Uuid::new_v4();
    let payment_id = Uuid::new_v4();

    assert_eq!(
        webhook_idempotency_key(merchant_id, payment_id, "Payment.Confirmed"),
        webhook_idempotency_key(merchant_id, payment_id, "payment.confirmed")
    );
}

#[test]
fn next_retry_at_applies_exponential_backoff() {
    let now = Utc::now();
    assert_eq!((next_retry_at(now, 3) - now).num_seconds(), 900);
}

#[test]
fn circuit_breaker_opens_only_for_repeated_5xx_failures() {
    let now = Utc::now();

    assert!(is_circuit_breaker_failure(Some(503)));
    assert!(!is_circuit_breaker_failure(Some(429)));
    assert!(!is_circuit_breaker_failure(None));

    assert_eq!(
        circuit_decision_after_failure(now, 4, Some(503), 5, 900),
        CircuitDecision::Closed
    );

    match circuit_decision_after_failure(now, 5, Some(503), 5, 900) {
        CircuitDecision::OpenUntil(until) => assert_eq!((until - now).num_seconds(), 900),
        CircuitDecision::Closed => panic!("expected circuit to open"),
    }
}

#[test]
fn worker_pool_size_tracks_queue_depth_without_exceeding_max() {
    assert_eq!(worker_pool_size(0, 16), 1);
    assert_eq!(worker_pool_size(3, 16), 3);
    assert_eq!(worker_pool_size(99, 16), 16);
}
