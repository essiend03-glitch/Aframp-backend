//! Replication lag monitor with circuit breaker (Issue #348).
//!
//! # Design
//! - Polls `pg_stat_replication` on the primary every `poll_interval`.
//! - Exposes the latest lag via `ReplicationMonitor::lag_secs()`.
//! - Circuit breaker: when lag exceeds `threshold_ms` the breaker opens and
//!   `is_open()` returns `true`, causing the router to fall back to the primary
//!   for all reads until lag recovers below `threshold_ms / 2` (hysteresis).
//! - Prometheus gauge `aframp_replication_lag_seconds{replica}` is updated on
//!   every poll.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tracing::{info, warn};

use crate::database::metrics as db_metrics;

/// Default lag threshold: 100 ms expressed in milliseconds.
pub const DEFAULT_THRESHOLD_MS: i64 = 100;

/// Hysteresis: breaker closes again when lag drops below this fraction of the threshold.
const HYSTERESIS_FACTOR: f64 = 0.5;

// ---------------------------------------------------------------------------
// ReplicationMonitor
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ReplicationMonitor {
    inner: Arc<Inner>,
}

struct Inner {
    /// Latest measured lag in milliseconds (-1 = unknown / no replica).
    lag_ms: AtomicI64,
    /// Circuit breaker state: true = open (replica unhealthy).
    breaker_open: AtomicBool,
    threshold_ms: i64,
    replica_label: String,
}

impl ReplicationMonitor {
    /// Create a monitor.  Call `spawn_poller` to start background polling.
    pub fn new(threshold_ms: i64, replica_label: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Inner {
                lag_ms: AtomicI64::new(-1),
                breaker_open: AtomicBool::new(false),
                threshold_ms,
                replica_label: replica_label.into(),
            }),
        }
    }

    /// Spawn a background task that polls the primary pool for replication lag.
    pub fn spawn_poller(&self, primary_pool: PgPool, poll_interval: Duration) {
        let monitor = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(poll_interval);
            loop {
                ticker.tick().await;
                monitor.poll_once(&primary_pool).await;
            }
        });
    }

    /// Perform a single lag measurement and update state.
    pub async fn poll_once(&self, primary_pool: &PgPool) {
        match measure_lag_ms(primary_pool).await {
            Ok(Some(lag_ms)) => {
                self.inner.lag_ms.store(lag_ms, Ordering::Relaxed);
                db_metrics::set_replication_lag(
                    &self.inner.replica_label,
                    lag_ms as f64 / 1000.0,
                );
                self.update_breaker(lag_ms);
            }
            Ok(None) => {
                // No replica connected — treat as healthy (single-node mode).
                self.inner.lag_ms.store(0, Ordering::Relaxed);
                db_metrics::set_replication_lag(&self.inner.replica_label, 0.0);
                self.inner.breaker_open.store(false, Ordering::Relaxed);
            }
            Err(e) => {
                warn!(replica=%self.inner.replica_label, "Replication lag query failed: {e}");
            }
        }
    }

    /// Current lag in milliseconds.  Returns -1 if not yet measured.
    pub fn lag_ms(&self) -> i64 {
        self.inner.lag_ms.load(Ordering::Relaxed)
    }

    /// Current lag in whole seconds (rounded down).
    pub fn lag_secs(&self) -> i64 {
        let ms = self.lag_ms();
        if ms < 0 { 0 } else { ms / 1000 }
    }

    /// True when the circuit breaker is open (lag exceeded threshold).
    /// Callers should route reads to the primary when this returns `true`.
    pub fn is_open(&self) -> bool {
        self.inner.breaker_open.load(Ordering::Relaxed)
    }

    // -----------------------------------------------------------------------
    // Private
    // -----------------------------------------------------------------------

    fn update_breaker(&self, lag_ms: i64) {
        let threshold = self.inner.threshold_ms;
        let currently_open = self.inner.breaker_open.load(Ordering::Relaxed);

        if !currently_open && lag_ms > threshold {
            warn!(
                replica=%self.inner.replica_label,
                lag_ms,
                threshold_ms=threshold,
                "Circuit breaker OPEN — replication lag exceeded threshold"
            );
            self.inner.breaker_open.store(true, Ordering::Relaxed);
        } else if currently_open {
            let close_threshold = (threshold as f64 * HYSTERESIS_FACTOR) as i64;
            if lag_ms <= close_threshold {
                info!(
                    replica=%self.inner.replica_label,
                    lag_ms,
                    close_threshold_ms=close_threshold,
                    "Circuit breaker CLOSED — replication lag recovered"
                );
                self.inner.breaker_open.store(false, Ordering::Relaxed);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Lag measurement
// ---------------------------------------------------------------------------

/// Query `pg_stat_replication` on the primary and return the worst-case lag
/// in milliseconds.  Returns `None` when no standby is connected.
pub async fn measure_lag_ms(
    primary_pool: &PgPool,
) -> Result<Option<i64>, sqlx::Error> {
    // write_lag is the most conservative lag metric (data not yet written to
    // the standby's WAL).  We take the MAX across all standbys.
    let row: Option<(Option<f64>,)> = sqlx::query_as(
        "SELECT MAX(EXTRACT(EPOCH FROM write_lag) * 1000)::float8 \
         FROM pg_stat_replication",
    )
    .fetch_optional(primary_pool)
    .await?;

    Ok(row.and_then(|(ms,)| ms.map(|v| v as i64)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_monitor(threshold_ms: i64) -> ReplicationMonitor {
        ReplicationMonitor::new(threshold_ms, "test-replica")
    }

    #[test]
    fn breaker_opens_when_lag_exceeds_threshold() {
        let m = make_monitor(100);
        m.update_breaker(150);
        assert!(m.is_open());
    }

    #[test]
    fn breaker_stays_closed_below_threshold() {
        let m = make_monitor(100);
        m.update_breaker(50);
        assert!(!m.is_open());
    }

    #[test]
    fn breaker_closes_with_hysteresis() {
        let m = make_monitor(100);
        // Open it
        m.update_breaker(200);
        assert!(m.is_open());
        // Lag drops to 60 ms — still above 50 ms (100 * 0.5) close threshold
        m.update_breaker(60);
        assert!(m.is_open(), "Should stay open above hysteresis threshold");
        // Lag drops to 40 ms — below 50 ms close threshold
        m.update_breaker(40);
        assert!(!m.is_open(), "Should close below hysteresis threshold");
    }

    #[test]
    fn lag_secs_converts_correctly() {
        let m = make_monitor(100);
        m.inner.lag_ms.store(2500, Ordering::Relaxed);
        assert_eq!(m.lag_secs(), 2);
    }

    #[test]
    fn unknown_lag_returns_zero_secs() {
        let m = make_monitor(100);
        // -1 is the "not yet measured" sentinel
        m.inner.lag_ms.store(-1, Ordering::Relaxed);
        assert_eq!(m.lag_secs(), 0);
    }
}
