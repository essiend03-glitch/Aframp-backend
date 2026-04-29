//! AML/KYC Compliance Effectiveness Reporting System
//!
//! Provides automated generation of compliance health reports covering:
//! - Alert volume (Sanctions/AML/KYC hits per period)
//! - False-positive rate (cleared LOW-risk cases)
//! - SLA compliance (alert-to-resolution time vs 24-hour target)
//! - Case disposition (cleared / blocked / pending)
//! - Risk distribution (LOW / MEDIUM / CRITICAL)
//! - Month-over-month trend analysis
//!
//! Reports are exportable as PDF (Typst), CSV, or JSON.
//! Scheduled generation is driven by `compliance_report_schedules`.
//! All access is RBAC-protected (compliance_officer / finance_director).

pub mod handlers;
pub mod models;
pub mod repository;
pub mod routes;
pub mod service;
pub mod worker;

pub use handlers::ComplianceEffectivenessState;
pub use models::{ComplianceMetrics, ComplianceReport, ReportFormat, ReportSchedule, ReportType};
pub use repository::ComplianceEffectivenessRepository;
pub use routes::compliance_effectiveness_routes;
pub use service::ReportGenerationService;
pub use worker::ComplianceReportWorker;
