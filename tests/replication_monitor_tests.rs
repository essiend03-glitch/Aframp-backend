//! Integration tests for replication lag circuit breaker and Read-Your-Writes routing.
//!
//! These are pure unit tests — no database connection required.

use std::time::Duration;

use aframp_backend::database::replication_monitor::ReplicationMonitor;
use aframp_backend::database::shard::RywTracker;

// ---------------------------------------------------------------------------
// Circuit breaker tests (public API only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod circuit_breaker {
    use super::*;

    fn monitor(threshold_ms: i64) -> ReplicationMonitor {
        ReplicationMonitor::new(threshold_ms, "test-replica")
    }

    #[test]
    fn starts_closed() {
        let m = monitor(100);
        assert!(!m.is_open());
    }

    #[test]
    fn lag_secs_zero_before_first_poll() {
        let m = monitor(100);
        assert_eq!(m.lag_secs(), 0);
    }
}

// ---------------------------------------------------------------------------
// Read-Your-Writes tracker tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod ryw {
    use super::*;

    #[tokio::test]
    async fn requires_primary_after_write() {
        let tracker = RywTracker::new(Duration::from_secs(30));
        tracker.record_write("session-abc").await;
        assert!(
            tracker.requires_primary("session-abc").await,
            "session must route to primary immediately after write"
        );
    }

    #[tokio::test]
    async fn does_not_require_primary_for_unknown_session() {
        let tracker = RywTracker::new(Duration::from_secs(30));
        assert!(
            !tracker.requires_primary("session-xyz").await,
            "unknown session must not require primary"
        );
    }

    #[tokio::test]
    async fn expires_after_ttl() {
        let tracker = RywTracker::new(Duration::from_millis(1));
        tracker.record_write("session-short").await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert!(
            !tracker.requires_primary("session-short").await,
            "session must not require primary after TTL expires"
        );
    }

    #[tokio::test]
    async fn write_refreshes_ttl() {
        let tracker = RywTracker::new(Duration::from_secs(30));
        tracker.record_write("session-refresh").await;
        tracker.record_write("session-refresh").await;
        assert!(tracker.requires_primary("session-refresh").await);
    }

    #[tokio::test]
    async fn evict_expired_removes_stale_sessions() {
        let tracker = RywTracker::new(Duration::from_millis(1));
        tracker.record_write("session-evict").await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        tracker.evict_expired().await;
        // After eviction, the session should no longer require primary.
        assert!(!tracker.requires_primary("session-evict").await);
    }

    #[tokio::test]
    async fn multiple_sessions_are_independent() {
        let tracker = RywTracker::new(Duration::from_secs(30));
        tracker.record_write("session-a").await;
        assert!(tracker.requires_primary("session-a").await);
        assert!(!tracker.requires_primary("session-b").await);
    }
}
