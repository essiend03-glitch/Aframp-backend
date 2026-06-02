//! Partner Revenue Sharing & Commission Management Engine (Issue #471).
//!
//! Provides split-fee evaluation, double-entry ledger, batch payout worker,
//! admin configuration endpoints, and partner revenue statement endpoints.

pub mod handlers;
pub mod metrics;
pub mod models;
pub mod repository;
pub mod routes;
pub mod service;
pub mod split_fee;
pub mod worker;

pub use models::{CommissionStructure, CommissionType, LedgerDirection, PayoutStatus};
pub use routes::commission_routes;
pub use service::CommissionService;
pub use split_fee::{CommissionBreakdown, SplitFeeEngine};
pub use worker::{PayoutWorker, PayoutWorkerConfig};
