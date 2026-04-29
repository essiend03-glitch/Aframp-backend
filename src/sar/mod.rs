//! Automated SAR (Suspicious Activity Report) workflow — Issue #420
//!
//! State machine: Draft → PendingReview → Approved → Filed → Acknowledged
//!
//! Triggered automatically by the AML engine on Critical/Medium flags.

pub mod handlers;
pub mod models;
pub mod repository;
pub mod routes;
pub mod service;
pub mod template;

pub use models::{ActivitySnapshot, RegulatoryAuthority, ReviewRequest, SarReport, SarStatus};
pub use service::SarService;
