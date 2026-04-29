//! PEP (Politically Exposed Person) Screening & Monitoring Engine — Issue #348
//!
//! Implements FATF-compliant PEP controls:
//! - Real-time screening during KYC onboarding via external PEP database
//! - Fuzzy name matching across aliases, transliterations, and language variants
//! - Tiered risk scoring (influence level × jurisdiction CPI × relationship type)
//! - Continuous nightly re-screening of the entire customer base
//! - Contextual false-positive filtering with configurable thresholds
//! - Automatic EDD task creation and senior management sign-off workflow
//! - Tamper-proof audit log of every screening event and manual decision

pub mod models;
pub mod screening;
pub mod risk_scoring;
pub mod monitoring;
pub mod repository;
pub mod handlers;
pub mod worker;

pub use models::{
    PepMatch, PepMatchStatus, PepRiskTier, PepRelationshipType, PepInfluenceLevel,
    PepScreeningRequest, PepScreeningResult, PepEddCase, PepEddStatus,
    PepAuditEntry, PepAuditAction, PepScreeningConfig,
};
pub use screening::PepScreeningService;
pub use risk_scoring::PepRiskScorer;
pub use monitoring::PepMonitoringService;
pub use repository::PepRepository;
pub use worker::PepRescreeningWorker;
