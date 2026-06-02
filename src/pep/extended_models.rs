//! Extended PEP data models for comprehensive PEP screening and monitoring
//! Issue #348 - Complete PEP Management System

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::{
    AssociationType, ConfidenceLevel, EddStatus, EddType, MonitoringFlag, PepCategory,
    PepProfileStatus, PepStatus, RelationshipType, ReviewStatus,
};

// ============================================================================
// PEP Profile - Central record for each identified PEP
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepProfile {
    pub pep_profile_id: Uuid,
    pub subject_kyc_id: Uuid,
    pub pep_category: PepCategory,
    pub pep_position_title: String,
    pub pep_organization: Option<String>,
    pub pep_country: String,
    pub pep_status: PepStatus,
    pub position_start_date: Option<NaiveDate>,
    pub position_end_date: Option<NaiveDate>,
    pub screening_source: String,
    pub match_confidence_score: u32,
    pub profile_status: PepProfileStatus,
    pub edd_status: EddStatus,
    pub assigned_compliance_officer: Option<Uuid>,
    pub created_timestamp: DateTime<Utc>,
    pub last_reviewed_timestamp: Option<DateTime<Utc>>,
    // Extended fields
    pub monitoring_start_date: Option<DateTime<Utc>>,
    pub monitoring_end_date: Option<DateTime<Utc>>,
    pub is_under_winddown: bool,
}

impl PepProfile {
    pub fn new(
        subject_kyc_id: Uuid,
        category: PepCategory,
        position_title: String,
        organization: Option<String>,
        country: String,
        source: String,
        confidence_score: u32,
    ) -> Self {
        Self {
            pep_profile_id: Uuid::new_v4(),
            subject_kyc_id,
            pep_category: category,
            pep_position_title: position_title,
            pep_organization: organization,
            pep_country: country,
            pep_status: PepStatus::Current,
            position_start_date: None,
            position_end_date: None,
            screening_source: source,
            match_confidence_score: confidence_score,
            profile_status: PepProfileStatus::UnderReview,
            edd_status: EddStatus::Pending,
            assigned_compliance_officer: None,
            created_timestamp: Utc::now(),
            last_reviewed_timestamp: None,
            monitoring_start_date: Some(Utc::now()),
            monitoring_end_date: None,
            is_under_winddown: false,
        }
    }

    /// Check if EDD is required based on confidence and category
    pub fn requires_edd(&self) -> bool {
        self.match_confidence_score >= 85
            || matches!(
                self.pep_category,
                PepCategory::DomesticPep | PepCategory::InternationalOrgPep
            )
    }

    /// Check if account should be blocked until EDD completion
    pub fn is_blocked_until_edd(&self) -> bool {
        matches!(
            self.profile_status,
            PepProfileStatus::Confirmed | PepProfileStatus::UnderReview
        ) && !matches!(self.edd_status, EddStatus::Completed)
    }

    /// Calculate next EDD renewal date
    pub fn next_renewal_date(&self) -> Option<NaiveDate> {
        let interval_days = if matches!(self.pep_status, PepStatus::Current) {
            365 // Annual for current PEPs
        } else {
            730 // Biennial for former PEPs
        };

        self.last_reviewed_timestamp
            .or(Some(self.created_timestamp))
            .map(|dt| dt.date_naive() + chrono::Duration::days(interval_days))
    }
}

// ============================================================================
// PEP Family Member
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepFamilyMember {
    pub family_member_id: Uuid,
    pub pep_profile_id: Uuid,
    pub family_member_kyc_id: Uuid,
    pub relationship_type: RelationshipType,
    pub screening_status: String,
    pub created_at: DateTime<Utc>,
}

impl PepFamilyMember {
    pub fn new(
        pep_profile_id: Uuid,
        family_member_kyc_id: Uuid,
        relationship_type: RelationshipType,
    ) -> Self {
        Self {
            family_member_id: Uuid::new_v4(),
            pep_profile_id,
            family_member_kyc_id,
            relationship_type,
            screening_status: "pending".to_string(),
            created_at: Utc::now(),
        }
    }
}

// ============================================================================
// PEP Close Associate
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepCloseAssociate {
    pub associate_id: Uuid,
    pub pep_profile_id: Uuid,
    pub associate_kyc_id: Uuid,
    pub association_type: AssociationType,
    pub screening_status: String,
    pub created_at: DateTime<Utc>,
}

impl PepCloseAssociate {
    pub fn new(
        pep_profile_id: Uuid,
        associate_kyc_id: Uuid,
        association_type: AssociationType,
    ) -> Self {
        Self {
            associate_id: Uuid::new_v4(),
            pep_profile_id,
            associate_kyc_id,
            association_type,
            screening_status: "pending".to_string(),
            created_at: Utc::now(),
        }
    }
}

// ============================================================================
// PEP EDD Record (Enhanced Due Diligence)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepEddRecord {
    pub edd_id: Uuid,
    pub pep_profile_id: Uuid,
    pub edd_type: EddType,
    pub edd_status: EddStatus,
    pub assigned_officer: Option<Uuid>,
    pub edd_findings: Option<String>,
    pub approval_status: String,
    pub approving_officer: Option<Uuid>,
    pub completion_timestamp: Option<DateTime<Utc>>,
    pub next_renewal_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PepEddRecord {
    pub fn new(pep_profile_id: Uuid, edd_type: EddType) -> Self {
        let now = Utc::now();
        Self {
            edd_id: Uuid::new_v4(),
            pep_profile_id,
            edd_type,
            edd_status: EddStatus::Pending,
            assigned_officer: None,
            edd_findings: None,
            approval_status: "pending".to_string(),
            approving_officer: None,
            completion_timestamp: None,
            next_renewal_date: Some(now.date_naive() + chrono::Duration::days(365)),
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if EDD is approved and completed
    pub fn is_complete(&self) -> bool {
        matches!(self.edd_status, EddStatus::Completed)
            && matches!(self.approval_status.as_str(), "approved")
    }

    /// Check if renewal is required
    pub fn is_renewal_due(&self) -> bool {
        if let Some(renewal_date) = self.next_renewal_date {
            renewal_date <= Utc::now().date_naive()
        } else {
            false
        }
    }
}

// ============================================================================
// PEP Transaction Monitoring Record
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepTransactionMonitoring {
    pub monitoring_id: Uuid,
    pub pep_profile_id: Uuid,
    pub transaction_id: Option<Uuid>,
    pub monitoring_flag: MonitoringFlag,
    pub review_status: ReviewStatus,
    pub reviewing_officer: Option<Uuid>,
    pub review_outcome: Option<String>,
    pub flagged_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

impl PepTransactionMonitoring {
    pub fn new(
        pep_profile_id: Uuid,
        transaction_id: Option<Uuid>,
        monitoring_flag: MonitoringFlag,
    ) -> Self {
        Self {
            monitoring_id: Uuid::new_v4(),
            pep_profile_id,
            transaction_id,
            monitoring_flag,
            review_status: ReviewStatus::Pending,
            reviewing_officer: None,
            review_outcome: None,
            flagged_at: Utc::now(),
            reviewed_at: None,
        }
    }

    /// Check if transaction requires mandatory review
    pub fn requires_mandatory_review(&self) -> bool {
        matches!(
            self.monitoring_flag,
            MonitoringFlag::ThresholdBreach | MonitoringFlag::RapidFundMovement
        )
    }
}

// ============================================================================
// PEP Database Version / Status
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepDatabaseVersion {
    pub version_id: Uuid,
    pub source_name: String,
    pub version_hash: String,
    pub entry_count: i32,
    pub ingested_at: DateTime<Utc>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepDatabaseStatus {
    pub id: Uuid,
    pub source_name: String,
    pub last_update: Option<DateTime<Utc>>,
    pub total_entries: i32,
    pub index_health: String,
    pub config: serde_json::Value,
}

impl PepDatabaseStatus {
    pub fn new(source_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_name,
            last_update: None,
            total_entries: 0,
            index_health: "healthy".to_string(),
            config: serde_json::json!({}),
        }
    }

    /// Check if database is stale (not updated within max staleness)
    pub fn is_stale(&self, max_staleness_hours: i64) -> bool {
        if let Some(last_update) = self.last_update {
            let hours_since_update = (Utc::now() - last_update).num_hours();
            hours_since_update > max_staleness_hours
        } else {
            true // Never updated
        }
    }
}

// ============================================================================
// PEP Screening Metrics
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepScreeningMetrics {
    pub metric_id: Uuid,
    pub metric_date: NaiveDate,
    pub total_screened: i32,
    pub pep_detections: i32,
    pub false_positives: i32,
    pub edd_initiated: i32,
    pub edd_completed: i32,
    pub transactions_flagged: i32,
    pub transactions_reviewed: i32,
    pub avg_detection_to_edd_completion_days: Option<f64>,
    pub created_at: DateTime<Utc>,
}

impl PepScreeningMetrics {
    pub fn new(date: NaiveDate) -> Self {
        Self {
            metric_id: Uuid::new_v4(),
            metric_date: date,
            total_screened: 0,
            pep_detections: 0,
            false_positives: 0,
            edd_initiated: 0,
            edd_completed: 0,
            transactions_flagged: 0,
            transactions_reviewed: 0,
            avg_detection_to_edd_completion_days: None,
            created_at: Utc::now(),
        }
    }

    /// Calculate detection rate
    pub fn detection_rate(&self) -> f64 {
        if self.total_screened > 0 {
            (self.pep_detections as f64 / self.total_screened as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Calculate false positive rate
    pub fn false_positive_rate(&self) -> f64 {
        let total_results = self.pep_detections + self.false_positives;
        if total_results > 0 {
            (self.false_positives as f64 / total_results as f64) * 100.0
        } else {
            0.0
        }
    }
}

// ============================================================================
// PEP Screening Result (Enhanced for new requirements)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedPepScreeningResult {
    pub consumer_id: Uuid,
    pub matches: Vec<PepScreeningMatch>,
    pub highest_confidence: Option<ConfidenceLevel>,
    pub pep_profile_created: bool,
    pub pep_profile_id: Option<Uuid>,
    pub routed_to_compliance_queue: bool,
    pub edd_initiated: bool,
    pub edd_case_id: Option<Uuid>,
    pub screened_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepScreeningMatch {
    pub match_id: Uuid,
    pub consumer_id: Uuid,
    pub matched_name: String,
    pub matched_aliases: Vec<String>,
    pub match_score: u8,
    pub confidence_level: ConfidenceLevel,
    // DOB and nationality matching
    pub dob_match: bool,
    pub nationality_match: bool,
    // PEP details
    pub pep_category: PepCategory,
    pub position_title: String,
    pub organization: Option<String>,
    pub country: String,
    pub status: PepStatus,
    // Risk assessment
    pub risk_tier: super::models::PepRiskTier,
    // Screening metadata
    pub screening_source: String,
    pub provider_entity_id: Option<String>,
    pub screened_at: DateTime<Utc>,
}

// ============================================================================
// API Request/Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFamilyMemberRequest {
    pub family_member_kyc_id: Uuid,
    pub relationship_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAssociateRequest {
    pub associate_kyc_id: Uuid,
    pub association_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiateEddRequest {
    pub edd_type: Option<String>,
    pub assigned_officer: Option<Uuid>,
    pub source_of_wealth: Option<String>,
    pub source_of_funds: Option<String>,
    pub business_purpose: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteEddRequest {
    pub officer_id: Uuid,
    pub findings: String,
    pub approval_status: String, // "approved" or "rejected"
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmPepRequest {
    pub reviewer_id: Uuid,
    pub notes: Option<String>,
    pub override_confidence: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearPepRequest {
    pub reviewer_id: Uuid,
    pub justification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewTransactionRequest {
    pub officer_id: Uuid,
    pub review_outcome: String, // "approved", "cleared", "escalate"
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileReviewRequest {
    pub reviewer_id: Uuid,
    pub findings: String,
    pub decision: String, // "continue" or "close"
    pub next_review_date: Option<NaiveDate>,
}

// ============================================================================
// Database Status API Response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepDatabaseStatusResponse {
    pub sources: Vec<PepDatabaseStatus>,
    pub overall_health: String,
    pub last_global_update: Option<DateTime<Utc>>,
    pub total_indexed_entries: i32,
}

// ============================================================================
// Monitoring Status API Response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringStatusResponse {
    pub job_status: String,
    pub progress: MonitoringProgress,
    pub last_completed: Option<DateTime<Utc>>,
    pub next_scheduled: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringProgress {
    pub total_consumers: i32,
    pub screened_count: i32,
    pub new_detections: i32,
    pub error_count: i32,
}

// ============================================================================
// Metrics API Response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepMetricsResponse {
    pub total_screened: i64,
    pub pep_detection_rate: f64,
    pub false_positive_rate: f64,
    pub edd_completion_rate: f64,
    pub avg_detection_to_edd_days: f64,
    pub active_pep_count: i64,
    pub pending_edd_count: i64,
    pub transaction_review_queue_depth: i64,
    pub days_since_last_db_update: Option<i64>,
}