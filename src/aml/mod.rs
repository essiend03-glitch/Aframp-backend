//! AML Financial Intelligence Layer — Cross-Border Transaction Screening
//!
//! Implements FATF-compliant AML controls for international payment corridors:
//! - Sanctions screening (OFAC, UN, EU SDN lists) via external AML provider
//! - Velocity & pattern analysis (smurfing, rapid-flip detection)
//! - Corridor-specific risk scoring (Basel AML Index / FATF Grey List)
//! - Automated case management with compliance officer workflow
//! - CTR (Currency Transaction Report) aggregation and threshold monitoring
//! - Automatic CTR generation on threshold breach
//! - CTR exemption management
//! - CTR review and approval workflow
//! - CTR document generation and regulatory filing
//! - CTR batch filing and deadline monitoring

pub mod models;
pub mod screening;
pub mod risk_scoring;
pub mod case_management;
pub mod enhanced_case_management;
pub mod policy_engine;
pub mod repository;
pub mod handlers;
pub mod ctr_aggregation;
pub mod ctr_generator;
pub mod ctr_exemption;
pub mod ctr_exemption_handlers;
pub mod ctr_management;
pub mod ctr_management_handlers;
pub mod ctr_filing;
pub mod ctr_filing_handlers;
pub mod ctr_batch_filing;
pub mod ctr_batch_filing_handlers;
pub mod ctr_reconciliation;
pub mod ctr_reconciliation_handlers;
pub mod ctr_metrics;
pub mod ctr_logging;

#[cfg(test)]
pub mod ctr_tests;

#[cfg(test)]
pub mod ctr_integration_tests;

pub use models::{
    AmlScreeningRequest, AmlScreeningResult, AmlFlag, AmlFlagLevel, AmlCaseStatus,
    CorridorRiskWeight, VelocityPattern, Ctr, CtrAggregation, CtrTransaction, CtrFiling,
    CtrExemption, CtrType, CtrStatus, DetectionMethod, TransactionDirection,
};
pub use screening::SanctionsScreeningService;
pub use risk_scoring::CorridorRiskScorer;
pub use case_management::AmlCaseManager;
pub use ctr_aggregation::{CtrAggregationService, CtrAggregationConfig, AggregationUpdateResult};
pub use ctr_generator::{CtrGeneratorService, CtrGeneratorConfig, CtrGenerationResult, SubjectInfo, TransactionInfo};
pub use ctr_exemption::{CtrExemptionService, CtrExemptionConfig, CreateExemptionRequest, ExemptionWithStatus, ExemptionCheckResult};
pub use ctr_exemption_handlers::{CtrExemptionState, create_exemption, get_exemptions, delete_exemption, get_expiring_exemptions};
pub use ctr_management::{CtrManagementService, CtrManagementConfig, ReviewChecklist, CtrReview, CtrApproval, CtrWithDetails, ReviewCtrRequest, ApproveCtrRequest, ReturnForCorrectionRequest};
pub use ctr_management_handlers::{CtrManagementState, get_ctrs, get_ctr_by_id, review_ctr, approve_ctr, return_for_correction, get_ctrs_requiring_senior_approval};
pub use ctr_filing::{CtrFilingService, CtrFilingConfig, CtrDocuments, FilingResult, FilingStatus, ValidationError};
pub use ctr_filing_handlers::{CtrFilingState, generate_documents, get_document, file_ctr};
pub use ctr_batch_filing::{CtrBatchFilingService, BatchFilingConfig, BatchFilingRequest, BatchFilingSummary, CtrDeadlineStatus, DeadlineStatusReport, ReminderNotification, ReminderType};
pub use ctr_batch_filing_handlers::{CtrBatchFilingState, batch_file_ctrs, get_deadline_status};
pub use ctr_reconciliation::{CtrReconciliationService, ReconciliationRequest, ReconciliationResult, ReconciliationDiscrepancy, MonthlyActivityReport, StatusBreakdown, TypeBreakdown, SubjectSummary, FilingPerformance};
pub use ctr_reconciliation_handlers::{CtrReconciliationState, reconcile_ctrs, get_monthly_report};
