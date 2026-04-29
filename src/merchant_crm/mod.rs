//! Merchant CRM & Customer Insights (Issue #334)
//!
//! Provides merchants with customer profiling, segmentation, purchasing pattern
//! analytics, and privacy-first data export capabilities.

pub mod models;
pub mod repository;
pub mod service;
pub mod handlers;
pub mod routes;
pub mod encryption;

pub use models::*;
pub use repository::CustomerProfileRepository;
pub use service::MerchantCrmService;
pub use routes::merchant_crm_routes;
