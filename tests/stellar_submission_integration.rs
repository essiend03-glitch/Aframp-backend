/// Integration tests for Stellar high-throughput submission engine
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use sqlx::PgPool;
    use uuid::Uuid;

    // Mock Horizon client for testing
    pub struct MockHorizonClient;

    // Helper to create test pool (requires DATABASE_URL in test environment)
    async fn get_test_pool() -> sqlx::PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/aframp_test".to_string());
        
        PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test --test stellar_submission_integration -- --ignored --nocapture
    async fn test_sequence_coordinator_concurrent_allocations() {
        use crate::stellar::sequence_coordinator::SequenceCoordinator;

        let coordinator = Arc::new(SequenceCoordinator::new(100, 100));
        let mut tasks = vec![];

        // Spawn 10 concurrent tasks, each reserving 10 sequences
        for _ in 0..10 {
            let coord = Arc::clone(&coordinator);
            let task = tokio::spawn(async move {
                let mut sequences = vec![];
                for _ in 0..10 {
                    if let Ok(seq) = coord.reserve_next() {
                        sequences.push(seq);
                    }
                }
                sequences
            });
            tasks.push(task);
        }

        let mut all_sequences = vec![];
        for task in tasks {
            all_sequences.extend(task.await.unwrap());
        }

        // All sequences should be unique
        all_sequences.sort_unstable();
        let mut prev = 0;
        for seq in all_sequences {
            assert!(seq > prev, "Duplicate or out-of-order sequence");
            prev = seq;
        }

        assert_eq!(prev, 200, "Should have reserved 100 sequences");
    }

    #[tokio::test]
    async fn test_fee_engine_surge_pricing() {
        use crate::stellar::fee_engine::DynamicFeeEngine;
        use crate::stellar::models::FeeConfiguration;

        let config = FeeConfiguration {
            base_fee: 100,
            min_fee: 100,
            max_fee: 10_000,
            surge_threshold: 0.8,
            surge_multiplier: 1.5,
            low_capacity_fee: 1_000,
        };

        let engine = DynamicFeeEngine::new(
            config,
            "https://horizon-testnet.stellar.org".to_string(),
        );

        // Test normal fee calculation
        let fee = engine.calculate_fee_with_multiplier(1, 100, 1.0);
        assert_eq!(fee, 100);

        // Test surge pricing
        let fee = engine.calculate_fee_with_multiplier(1, 100, 1.5);
        assert_eq!(fee, 150);

        // Test multi-op fees
        let fee = engine.calculate_fee_with_multiplier(5, 100, 1.0);
        assert_eq!(fee, 500);

        // Test fee capping
        let fee = engine.calculate_fee_with_multiplier(100, 100, 2.0);
        assert_eq!(fee, 10_000); // Should be capped
    }

    #[tokio::test]
    async fn test_retry_state_machine_exponential_backoff() {
        use crate::stellar::retry_state_machine::RetryStateMachine;
        use crate::stellar::models::RetryPolicy;
        use crate::stellar::error::SubmissionError;
        use std::time::Duration;

        let policy = RetryPolicy {
            max_retries: 5,
            base_backoff_ms: 100,
            max_backoff_ms: 10_000,
            backoff_multiplier: 2.0,
        };

        let mut machine = RetryStateMachine::new(policy);
        let error = SubmissionError::TransientNetworkError {
            source: "timeout".to_string(),
            attempt: 1,
        };

        // First retry should have base backoff
        let delay1 = machine.calculate_next_retry_delay();
        assert_eq!(delay1.as_millis(), 100);

        // Simulate recording attempt
        machine.record_attempt(&error).unwrap();

        // Subsequent retries should double
        let delay2 = machine.calculate_next_retry_delay();
        assert!(delay2.as_millis() >= 200, "Backoff should double");
    }

    #[tokio::test]
    async fn test_retry_state_machine_channel_rotation_trigger() {
        use crate::stellar::retry_state_machine::RetryStateMachine;
        use crate::stellar::models::RetryPolicy;
        use crate::stellar::error::SubmissionError;

        let policy = RetryPolicy::default();
        let machine = RetryStateMachine::new(policy);

        // Bad sequence should trigger rotation
        let error = SubmissionError::BadSequence("mismatch".to_string());
        assert!(machine.should_rotate_channel(&error));

        // Channel exhaustion should trigger rotation
        let error = SubmissionError::ChannelExhausted("no slots".to_string());
        assert!(machine.should_rotate_channel(&error));

        // Transient errors should not trigger rotation
        let error = SubmissionError::TransientNetworkError {
            source: "timeout".to_string(),
            attempt: 1,
        };
        assert!(!machine.should_rotate_channel(&error));
    }

    #[tokio::test]
    async fn test_retry_state_machine_stale_detection() {
        use crate::stellar::retry_state_machine::RetryStateMachine;
        use crate::stellar::models::RetryPolicy;
        use chrono::Duration;

        let policy = RetryPolicy::default();
        let mut machine = RetryStateMachine::new(policy);

        // Manually set creation time to be stale
        machine.created_at = chrono::Utc::now() - Duration::seconds(30);

        // Should be detected as stale with 5 second threshold
        assert!(machine.is_stale(Duration::seconds(5)));

        // Should not be stale with 60 second threshold
        assert!(!machine.is_stale(Duration::seconds(60)));
    }

    #[tokio::test]
    #[ignore] // Requires test database
    async fn test_channel_pool_load_balancing() {
        use crate::stellar::channel_pool::ChannelPool;

        let pool = get_test_pool().await;
        let issuer_id = Uuid::new_v4();

        // This would require test database setup
        // Tested in integration tests with real DB
    }

    #[tokio::test]
    async fn test_error_classification() {
        use crate::stellar::error::{SubmissionError, HorizonErrorCode};

        let bad_seq_error = SubmissionError::BadSequence("mismatch".to_string());
        let code = HorizonErrorCode::from_str("tx_bad_seq");
        assert!(matches!(code, HorizonErrorCode::TxBadSeq));

        let fee_error = SubmissionError::InsufficientFee {
            provided: 100,
            required: 1000,
        };
        let code = HorizonErrorCode::from_str("tx_insufficient_fee");
        assert!(matches!(code, HorizonErrorCode::TxInsufficientFee));
    }

    #[tokio::test]
    async fn test_error_retryability() {
        use crate::stellar::error::HorizonErrorCode;

        let transient = HorizonErrorCode::Transient;
        assert!(transient.is_retryable());

        let bad_seq = HorizonErrorCode::TxBadSeq;
        assert!(!bad_seq.is_retryable());

        let stale = HorizonErrorCode::StaleLedgerVersion;
        assert!(stale.is_retryable());
    }

    #[tokio::test]
    async fn test_channel_exhaustion_detection() {
        use crate::stellar::error::HorizonErrorCode;

        let bad_seq = HorizonErrorCode::TxBadSeq;
        assert!(bad_seq.is_channel_exhaustion());

        let insufficient_fee = HorizonErrorCode::TxInsufficientFee;
        assert!(insufficient_fee.is_channel_exhaustion());

        let transient = HorizonErrorCode::Transient;
        assert!(!transient.is_channel_exhaustion());
    }

    #[test]
    fn test_sequence_coordinator_basic_operations() {
        use crate::stellar::sequence_coordinator::SequenceCoordinator;

        let coordinator = SequenceCoordinator::new(100, 10);

        // Initial state
        assert_eq!(coordinator.current_sequence(), 100);
        assert_eq!(coordinator.reserved_sequence(), 100);
        assert_eq!(coordinator.in_flight_count(), 0);

        // Reserve sequence
        let seq1 = coordinator.reserve_next().unwrap();
        assert_eq!(seq1, 101);
        assert_eq!(coordinator.in_flight_count(), 1);

        // Mark confirmed
        coordinator.mark_confirmed(101).unwrap();
        assert_eq!(coordinator.current_sequence(), 101);
        assert_eq!(coordinator.in_flight_count(), 0);
    }

    #[test]
    fn test_sequence_coordinator_exhaustion() {
        use crate::stellar::sequence_coordinator::SequenceCoordinator;

        let coordinator = SequenceCoordinator::new(100, 2);

        coordinator.reserve_next().unwrap();
        coordinator.reserve_next().unwrap();

        // Should be exhausted
        let result = coordinator.reserve_next();
        assert!(result.is_err());
    }

    #[test]
    fn test_fee_engine_bounds() {
        use crate::stellar::fee_engine::DynamicFeeEngine;
        use crate::stellar::models::FeeConfiguration;

        let config = FeeConfiguration {
            base_fee: 100,
            min_fee: 100,
            max_fee: 5_000,
            surge_threshold: 0.8,
            surge_multiplier: 2.0,
            low_capacity_fee: 1_000,
        };

        let engine = DynamicFeeEngine::new(config, "".to_string());

        // Test that fees respect min/max bounds
        let fee_below_min = engine.calculate_fee_with_multiplier(1, 50, 0.5);
        assert_eq!(fee_below_min, 100); // Should floor to min

        let fee_above_max = engine.calculate_fee_with_multiplier(100, 100, 2.0);
        assert_eq!(fee_above_max, 5_000); // Should cap to max
    }
}
