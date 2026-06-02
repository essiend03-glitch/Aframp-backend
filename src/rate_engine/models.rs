use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Loaded from `tenant_sla_profiles`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSlaProfile {
    pub tenant_id:            Uuid,
    pub tier:                 String,
    pub max_concurrent_conns: i32,
    pub baseline_rps:         i32,
    pub burst_rps:            i32,
    pub queue_weight:         i32,
    pub burst_window_ms:      i32,
    pub enabled:              bool,
}

impl TenantSlaProfile {
    /// Replenishment rate (tokens/second) == baseline_rps.
    pub fn fill_rate(&self) -> f64 { self.baseline_rps as f64 }
    /// Maximum burst capacity.
    pub fn capacity(&self)  -> f64 { self.burst_rps    as f64 }
}

/// Decision returned by the rate-limiter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allow,
    /// Caller should wait this many milliseconds before retrying.
    Throttle { retry_after_ms: u64 },
}
