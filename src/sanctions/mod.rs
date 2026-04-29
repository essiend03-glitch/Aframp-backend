//! Sanctions screening engine — Issue #419
//!
//! Provides real-time, blocking-at-the-edge screening of every strong-consistency
//! transaction against OFAC, UN, EU, and local watchlists.
//!
//! # Modules
//! - `models`    — shared data types
//! - `screener`  — core screening logic, fuzzy matching, provider integration
//! - `audit_log` — immutable append-only screening log
//! - `bypass`    — dual-authorisation bypass for compliance overrides

pub mod audit_log;
pub mod bypass;
pub mod models;
pub mod screener;

#[cfg(test)]
mod tests;

pub use audit_log::AuditLog;
pub use bypass::{BypassError, BypassService};
pub use models::{
    BypassRequest, ScreeningLogEntry, ScreeningOutcome, ScreeningRequest, ScreeningResult,
    SanctionsMatch, WatchlistSource,
};
pub use screener::{fuzzy_score, normalise, ScreenerConfig, SanctionsScreener};
