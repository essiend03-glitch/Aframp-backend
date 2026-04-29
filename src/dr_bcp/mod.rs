//! Disaster Recovery & Business Continuity Planning (DR/BCP) Module (Issue #DR-BCP).
//!
//! Formalises the "Playbook for Survival" covering:
//!   - Business Impact Analysis (BIA) with Maximum Tolerable Downtime (MTD) per service
//!   - Immutable backup management and automated restore verification
//!   - RPO < 0 min (zero data loss) and RTO < 15 min targets
//!   - Emergency Response Team (ERT) structure and automated notifications
//!   - Regulatory communication templates (CBN, SEC, partner FIs)
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │                    DrBcpService                              │
//! │                                                              │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
//! │  │  BiaRegistry │  │ BackupManager│  │  IncidentCommand │  │
//! │  │  (MTD map)   │  │ (immutable)  │  │  (ERT + notify)  │  │
//! │  └──────────────┘  └──────────────┘  └──────────────────┘  │
//! │                                                              │
//! │  ┌──────────────┐  ┌──────────────┐                         │
//! │  │ RpoRtoTracker│  │ RegulatoryMgr│                         │
//! │  │ (metrics)    │  │ (templates)  │                         │
//! │  └──────────────┘  └──────────────┘                         │
//! └──────────────────────────────────────────────────────────────┘
//! ```

pub mod handlers;
pub mod models;
pub mod repository;
pub mod routes;
pub mod service;

pub use models::*;
pub use repository::DrBcpRepository;
pub use routes::dr_bcp_routes;
pub use service::DrBcpService;
