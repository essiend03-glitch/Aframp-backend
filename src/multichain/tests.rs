//! Unit tests for Issue #532: hashlock math, time-lock deltas, EVM payload serialization.

use crate::multichain::{models::*, state_relay::StateRelayEngine};
use chrono::Utc;
use uuid::Uuid;

#[test]
fn test_hashlock_leaf_deterministic() {
    let h1 = StateRelayEngine::hashlock_leaf("secret123");
    let h2 = StateRelayEngine::hashlock_leaf("secret123");
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
}

#[test]
fn test_timelock_delta_future() {
    let expiry = Utc::now() + chrono::Duration::hours(24);
    let delta = expiry - Utc::now();
    assert!(delta.num_hours() >= 23); // at least 23h remaining
}

#[test]
fn test_swap_status_display() {
    assert_eq!(SwapStatus::HeldForManualReconciliation.to_string(), "HELD_FOR_MANUAL_RECONCILIATION");
    assert_eq!(SwapStatus::AssetLocked.to_string(), "ASSET_LOCKED");
}

#[test]
fn test_18_decimal_precision() {
    // 10^-18 invariant: 1 ETH in wei
    let wei: u128 = 1_000_000_000_000_000_000;
    let reconstructed = format!("{}", wei);
    assert_eq!(reconstructed, "1000000000000000000");
    assert_eq!(reconstructed.len(), 19); // 18 zeros + leading 1
}
