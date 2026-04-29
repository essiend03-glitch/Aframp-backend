//! Region detection and read-replica routing (Issue #348).
//!
//! The gateway reads `REGION` from the environment (injected by the container
//! orchestrator / ECS task definition) and selects the appropriate database URL:
//!   - Strong-consistency requests → `DATABASE_URL` (primary, us-east-1)
//!   - Eventual-consistency requests → `DATABASE_READ_REPLICA_URL` (local replica)
//!
//! DNS-based latency routing (Route 53 / Anycast) directs traffic to the nearest
//! region; this module handles the per-request DB connection selection.

use std::env;

/// AWS region this instance is running in (e.g. "us-east-1", "eu-west-1").
pub fn current_region() -> String {
    env::var("REGION").unwrap_or_else(|_| "us-east-1".to_owned())
}

/// Returns `true` when the request header `X-Consistency: strong` is present,
/// or when the path is known to require strong consistency (financial writes).
pub fn requires_strong_consistency(path: &str, consistency_header: Option<&str>) -> bool {
    if consistency_header
        .map(|v| v.eq_ignore_ascii_case("strong"))
        .unwrap_or(false)
    {
        return true;
    }
    // Paths that always need the primary DB regardless of header.
    path.starts_with("/account/")
        || path.contains("/transaction")
        || path.contains("/mint")
        || path.contains("/redemption")
        || path.contains("/transfer")
}

/// Select the database URL for this request.
///
/// Returns the read-replica URL for eventual-consistency reads, falling back to
/// the primary URL if no replica is configured.
pub fn select_db_url(strong: bool) -> String {
    if strong {
        env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    } else {
        env::var("DATABASE_READ_REPLICA_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .expect("DATABASE_URL must be set")
    }
}
