/// Lock-free sequence number coordinator for parallel transaction submissions
/// 
/// Maintains atomic counters for current and reserved sequence numbers across
/// multiple Tokio threads. Prevents duplicate sequence number exceptions through
/// atomic compare-and-swap operations.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use crate::stellar::error::{SubmissionError, SubmissionResult};

/// Coordinates sequence number allocation for a single channel account
#[derive(Debug, Clone)]
pub struct SequenceCoordinator {
    /// Current confirmed sequence number on-chain
    current: Arc<AtomicI64>,
    
    /// Reserved sequence number for in-flight transactions
    reserved: Arc<AtomicI64>,
    
    /// Maximum allowed in-flight transactions per channel
    max_in_flight: u32,
}

impl SequenceCoordinator {
    /// Create a new sequence coordinator
    pub fn new(initial_sequence: i64, max_in_flight: u32) -> Self {
        Self {
            current: Arc::new(AtomicI64::new(initial_sequence)),
            reserved: Arc::new(AtomicI64::new(initial_sequence)),
            max_in_flight,
        }
    }

    /// Reserve the next sequence number for a new transaction
    /// Returns the reserved sequence if successful, or error if exhausted
    pub fn reserve_next(&self) -> SubmissionResult<i64> {
        let mut retries = 0;
        let max_retries = 100;

        loop {
            let current_reserved = self.reserved.load(Ordering::SeqCst);
            let current_current = self.current.load(Ordering::SeqCst);

            // Calculate in-flight transactions
            let in_flight = current_reserved - current_current;
            if in_flight >= self.max_in_flight as i64 {
                return Err(SubmissionError::SequenceCoordinatorError(
                    format!(
                        "channel exhausted: {} in-flight txns (max: {})",
                        in_flight, self.max_in_flight
                    ),
                ));
            }

            let next_reserved = current_reserved + 1;

            // Try to claim this sequence atomically
            match self.reserved.compare_exchange(
                current_reserved,
                next_reserved,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Ok(current_reserved + 1),
                Err(_) => {
                    retries += 1;
                    if retries >= max_retries {
                        return Err(SubmissionError::SequenceCoordinatorError(
                            "unable to reserve sequence after max retries".to_string(),
                        ));
                    }
                    // Backoff and retry
                    std::thread::yield_now();
                }
            }
        }
    }

    /// Mark a sequence number as confirmed (successfully submitted and on-chain)
    pub fn mark_confirmed(&self, sequence: i64) -> SubmissionResult<()> {
        // Update current to the highest confirmed sequence
        let mut current_val = self.current.load(Ordering::SeqCst);

        loop {
            if sequence <= current_val {
                // Already confirmed or stale
                return Ok(());
            }

            match self.current.compare_exchange(
                current_val,
                sequence,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => {
                    current_val = actual;
                    if sequence <= current_val {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Get current confirmed sequence
    pub fn current_sequence(&self) -> i64 {
        self.current.load(Ordering::SeqCst)
    }

    /// Get reserved sequence (for diagnostics)
    pub fn reserved_sequence(&self) -> i64 {
        self.reserved.load(Ordering::SeqCst)
    }

    /// Get number of in-flight transactions
    pub fn in_flight_count(&self) -> i64 {
        self.reserved.load(Ordering::SeqCst) - self.current.load(Ordering::SeqCst)
    }

    /// Atomically update current sequence to a new value from Horizon
    /// This handles the case where Horizon returns a different sequence than expected
    pub fn sync_with_horizon(&self, horizon_sequence: i64) -> SubmissionResult<()> {
        let current_val = self.current.load(Ordering::SeqCst);
        let reserved_val = self.reserved.load(Ordering::SeqCst);

        if horizon_sequence < current_val {
            // Horizon is behind our tracking - likely a reorg or we're ahead
            // This shouldn't happen in normal operation
            return Err(SubmissionError::SequenceCoordinatorError(
                format!(
                    "Horizon sequence {} is behind current {}",
                    horizon_sequence, current_val
                ),
            ));
        }

        if horizon_sequence > reserved_val {
            // Horizon is ahead of our reserved - update both
            self.current.store(horizon_sequence, Ordering::SeqCst);
            self.reserved.store(horizon_sequence, Ordering::SeqCst);
        } else if horizon_sequence > current_val {
            // Normal case: Horizon has confirmed up to horizon_sequence
            self.current.store(horizon_sequence, Ordering::SeqCst);
        }

        Ok(())
    }

    /// Reset sequence numbers (use with caution - for testing/recovery only)
    #[cfg(test)]
    pub fn reset(&self, new_sequence: i64) {
        self.current.store(new_sequence, Ordering::SeqCst);
        self.reserved.store(new_sequence, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reserve_next_increments() {
        let coordinator = SequenceCoordinator::new(100, 10);
        assert_eq!(coordinator.reserve_next().unwrap(), 101);
        assert_eq!(coordinator.reserve_next().unwrap(), 102);
        assert_eq!(coordinator.reserved_sequence(), 102);
        assert_eq!(coordinator.current_sequence(), 100);
    }

    #[test]
    fn test_reserve_exhaustion() {
        let coordinator = SequenceCoordinator::new(100, 2);
        assert_eq!(coordinator.reserve_next().unwrap(), 101);
        assert_eq!(coordinator.reserve_next().unwrap(), 102);
        assert!(coordinator.reserve_next().is_err());
    }

    #[test]
    fn test_mark_confirmed() {
        let coordinator = SequenceCoordinator::new(100, 10);
        coordinator.reserve_next().unwrap();
        coordinator.reserve_next().unwrap();
        coordinator.mark_confirmed(101).unwrap();
        coordinator.mark_confirmed(102).unwrap();
        assert_eq!(coordinator.current_sequence(), 102);
    }

    #[test]
    fn test_parallel_reserve() {
        use std::sync::Arc;
        use std::thread;

        let coordinator = Arc::new(SequenceCoordinator::new(100, 100));
        let mut handles = vec![];

        for _ in 0..10 {
            let coord = coordinator.clone();
            let handle = thread::spawn(move || {
                let mut sequences = vec![];
                for _ in 0..10 {
                    if let Ok(seq) = coord.reserve_next() {
                        sequences.push(seq);
                    }
                }
                sequences
            });
            handles.push(handle);
        }

        let mut all_sequences = vec![];
        for handle in handles {
            all_sequences.extend(handle.join().unwrap());
        }

        // All sequences should be unique
        all_sequences.sort_unstable();
        let mut prev = 0;
        for seq in all_sequences {
            assert!(seq > prev, "Duplicate or out-of-order sequence");
            prev = seq;
        }
    }

    #[test]
    fn test_sync_with_horizon() {
        let coordinator = SequenceCoordinator::new(100, 10);
        coordinator.reserve_next().unwrap();
        coordinator.reserve_next().unwrap();

        // Horizon returns that sequence 102 is confirmed
        coordinator.sync_with_horizon(102).unwrap();
        assert_eq!(coordinator.current_sequence(), 102);
        assert_eq!(coordinator.reserved_sequence(), 102);
    }
}
