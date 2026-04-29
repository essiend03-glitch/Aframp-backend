//! Multi-Store & Franchise Management (Issue #335).
//!
//! Provides a parent-child account hierarchy: Organization → Region → Branch,
//! with RBAC, consolidated settlement, and cross-store analytics.

pub mod models;
pub mod repository;
pub mod service;
pub mod handlers;
pub mod routes;
pub mod rbac;

pub use models::*;
pub use repository::FranchiseRepository;
pub use service::FranchiseService;
pub use routes::franchise_routes;
