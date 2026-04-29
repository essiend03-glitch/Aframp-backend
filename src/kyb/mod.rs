//! KYB (Know Your Business) — Corporate Entity Verification
//!
//! Pipeline: Draft → Documents Submitted → Registry Verified → Compliance Review → Approved/Rejected
//!
//! - Corporate registry integration (CAC Nigeria + mock fallback)
//! - UBO extraction (>= 25% ownership) with automatic KYC trigger
//! - Risk scoring (industry / jurisdiction / registry status)
//! - Encrypted document storage (AES-256-GCM) with OCR name validation
//! - State-machine orchestrator

pub mod document_store;
pub mod handlers;
pub mod models;
pub mod orchestrator;
pub mod registry;
pub mod repository;
pub mod risk_scoring;
pub mod routes;
pub mod ubo;

pub use handlers::KybState;
pub use orchestrator::KybOrchestrator;
pub use repository::KybRepository;
pub use routes::kyb_routes;
