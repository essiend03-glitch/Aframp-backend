//! Merchant Invoicing & Automated Tax Calculation (Issue #333).
//!
//! Provides dynamic tax engine, automated invoice generation, accounting
//! software integration, and FIRS-formatted tax collection reports.

pub mod models;
pub mod repository;
pub mod tax_engine;
pub mod service;
pub mod handlers;
pub mod routes;

pub use models::*;
pub use repository::InvoicingRepository;
pub use service::MerchantInvoicingService;
pub use routes::merchant_invoicing_routes;
