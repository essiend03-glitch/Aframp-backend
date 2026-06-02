//! Regulatory Examination Support & Evidence Package (Issue #348-ext)
//!
//! Provides:
//! - Automated evidence collection from AML, Travel Rule, KYC, and Multisig sources
//! - Point-in-time policy history (e.g. "What was our KYC threshold on Jan 1?")
//! - Cryptographically signed (HMAC-SHA256) evidence package exports
//! - System health & test report attachment
//! - All generation requests logged to the Immutable Audit Trail

pub mod handlers;
pub mod models;
pub mod repository;
pub mod routes;
pub mod service;

pub use handlers::RegulatoryEvidenceState;
pub use repository::RegulatoryEvidenceRepository;
pub use routes::regulatory_evidence_routes;
pub use service::RegulatoryEvidenceService;
