/// Async retry state machine for Stellar transaction submissions
///
/// Implements exponential backoff, channel rotation on sequence errors,
/// and graceful degradation on transient failures.

use crate::stellar::error::{SubmissionError, SubmissionResult, HorizonErrorCode};
use crate::stellar::models::{RetryPolicy, TransactionLogEntry};
use chrono::{DateTime, Utc, Duration};
use std::time::Duration as StdDuration;

/// Current state of a retry attempt
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryState {
    /// Transaction is pending initial submission
    Pending,
    /// Currently retrying after a transient error
    Retrying { attempt: u32 },
    /// Confirmed on-chain
    Confirmed { stellar_tx_hash: String, ledger: i64 },
    /// Failed permanently
    Failed { reason: String },
    /// Stale - stuck beyond recovery window
    Stale { since: DateTime<Utc> },
}

/// Retry state machine for a single transaction
pub struct RetryStateMachine {
    policy: RetryPolicy,
    state: RetryState,
    created_at: DateTime<Utc>,
    last_attempt_at: Option<DateTime<Utc>>,
    next_retry_at: Option<DateTime<Utc>>,
    error_history: Vec<(DateTime<Utc>, String)>,
}

impl RetryStateMachine {
    /// Create a new retry state machine
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            state: RetryState::Pending,
            created_at: Utc::now(),
            last_attempt_at: None,
            next_retry_at: None,
            error_history: Vec::new(),
        }
    }

    /// Check if we should retry this error
    pub fn should_retry(&self, error: &SubmissionError) -> bool {
        match self.get_retry_count() {
            count if count >= self.policy.max_retries => false,
            _ => {
                // Check if error is retryable
                match error {
                    SubmissionError::TransientNetworkError { .. } => true,
                    SubmissionError::BadSequence(_) => true,
                    SubmissionError::InsufficientFee { .. } => true,
                    SubmissionError::HorizonApi(msg) => {
                        let code = HorizonErrorCode::from_str(msg);
                        code.is_retryable()
                    }
                    _ => false,
                }
            }
        }
    }

    /// Calculate next retry delay using exponential backoff
    pub fn calculate_next_retry_delay(&self) -> StdDuration {
        let retry_count = self.get_retry_count();
        let backoff_ms = (self.policy.base_backoff_ms as f64
            * self.policy.backoff_multiplier.powi(retry_count as i32))
            .min(self.policy.max_backoff_ms as f64) as u64;

        StdDuration::from_millis(backoff_ms)
    }

    /// Record a failed attempt and calculate next retry time
    pub fn record_attempt(&mut self, error: &SubmissionError) -> SubmissionResult<()> {
        self.last_attempt_at = Some(Utc::now());
        self.error_history
            .push((Utc::now(), error.to_string()));

        if self.should_retry(error) {
            let delay = self.calculate_next_retry_delay();
            self.next_retry_at = Some(Utc::now() + Duration::from_std(delay)?);
            self.state = RetryState::Retrying {
                attempt: self.get_retry_count() + 1,
            };
        } else {
            self.state = RetryState::Failed {
                reason: error.to_string(),
            };
        }

        Ok(())
    }

    /// Mark transaction as confirmed
    pub fn mark_confirmed(&mut self, stellar_tx_hash: String, ledger: i64) {
        self.state = RetryState::Confirmed {
            stellar_tx_hash,
            ledger,
        };
        self.next_retry_at = None;
    }

    /// Check if transaction is stale (stuck beyond recovery)
    pub fn is_stale(&self, stale_threshold: Duration) -> bool {
        let age = Utc::now() - self.created_at;
        age > stale_threshold
    }

    /// Mark transaction as stale
    pub fn mark_stale(&mut self) {
        self.state = RetryState::Stale {
            since: Utc::now(),
        };
        self.next_retry_at = None;
    }

    /// Get current retry count
    pub fn get_retry_count(&self) -> u32 {
        match &self.state {
            RetryState::Retrying { attempt } => *attempt,
            _ => self.error_history.len() as u32,
        }
    }

    /// Check if ready for next retry
    pub fn is_ready_for_retry(&self) -> bool {
        match &self.state {
            RetryState::Retrying { .. } => {
                if let Some(next_retry) = self.next_retry_at {
                    Utc::now() >= next_retry
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Get current state
    pub fn current_state(&self) -> &RetryState {
        &self.state
    }

    /// Get error history
    pub fn error_history(&self) -> &[(DateTime<Utc>, String)] {
        &self.error_history
    }

    /// Get age of this transaction
    pub fn age(&self) -> Duration {
        Utc::now() - self.created_at
    }

    /// Should we rotate channels for this error?
    pub fn should_rotate_channel(&self, error: &SubmissionError) -> bool {
        match error {
            SubmissionError::BadSequence(_) => true,
            SubmissionError::ChannelExhausted(_) => true,
            SubmissionError::SequenceCoordinatorError(_) => true,
            _ => false,
        }
    }

    /// Classify error for metrics/alerts
    pub fn classify_error(&self, error: &SubmissionError) -> ErrorClassification {
        match error {
            SubmissionError::TransientNetworkError { .. } => ErrorClassification::Transient,
            SubmissionError::BadSequence(_) => ErrorClassification::SequenceError,
            SubmissionError::InsufficientFee { .. } => ErrorClassification::FeeError,
            SubmissionError::ChannelExhausted(_) => ErrorClassification::ChannelExhausted,
            _ => ErrorClassification::Other,
        }
    }
}

/// Classification of errors for metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClassification {
    Transient,
    SequenceError,
    FeeError,
    ChannelExhausted,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_machine() -> RetryStateMachine {
        let policy = RetryPolicy {
            max_retries: 5,
            base_backoff_ms: 100,
            max_backoff_ms: 10_000,
            backoff_multiplier: 2.0,
        };
        RetryStateMachine::new(policy)
    }

    #[test]
    fn test_initial_state_is_pending() {
        let machine = create_test_machine();
        assert_eq!(machine.current_state(), &RetryState::Pending);
    }

    #[test]
    fn test_should_retry_transient_error() {
        let machine = create_test_machine();
        let error = SubmissionError::TransientNetworkError {
            source: "timeout".to_string(),
            attempt: 1,
        };
        assert!(machine.should_retry(&error));
    }

    #[test]
    fn test_should_not_retry_after_max_attempts() {
        let mut machine = create_test_machine();
        let error = SubmissionError::TransientNetworkError {
            source: "timeout".to_string(),
            attempt: 1,
        };

        // Simulate max retries
        for _ in 0..6 {
            machine.record_attempt(&error).unwrap();
        }

        assert!(!machine.should_retry(&error));
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let machine = create_test_machine();
        
        let delay_1 = machine.calculate_next_retry_delay().as_millis();
        assert_eq!(delay_1, 100);
    }

    #[test]
    fn test_is_stale_detection() {
        let mut machine = create_test_machine();
        
        // Not stale initially
        assert!(!machine.is_stale(Duration::seconds(5)));
        
        // Manually set created time to stale
        machine.created_at = Utc::now() - Duration::seconds(30);
        assert!(machine.is_stale(Duration::seconds(5)));
    }

    #[test]
    fn test_should_rotate_channel_on_bad_sequence() {
        let machine = create_test_machine();
        let error = SubmissionError::BadSequence("mismatch".to_string());
        assert!(machine.should_rotate_channel(&error));
    }

    #[test]
    fn test_mark_confirmed() {
        let mut machine = create_test_machine();
        machine.mark_confirmed("hash123".to_string(), 42);
        
        match machine.current_state() {
            RetryState::Confirmed { stellar_tx_hash, ledger } => {
                assert_eq!(stellar_tx_hash, "hash123");
                assert_eq!(*ledger, 42);
            }
            _ => panic!("expected confirmed state"),
        }
    }
}
