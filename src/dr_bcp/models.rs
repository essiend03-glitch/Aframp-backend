//! Data models for Disaster Recovery & Business Continuity Planning (Issue #DR-BCP).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// Criticality tier for Business Impact Analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "service_criticality", rename_all = "snake_case")]
pub enum ServiceCriticality {
    /// MTD in minutes — Stellar settlement, payment processing.
    Critical,
    /// MTD in hours — KYC, compliance monitoring.
    High,
    /// MTD in days — reporting, analytics.
    Low,
}

/// Lifecycle status of a DR incident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "dr_incident_status", rename_all = "snake_case")]
pub enum DrIncidentStatus {
    Declared,
    Active,
    Recovering,
    Resolved,
    PostMortemPending,
}

impl DrIncidentStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Resolved | Self::PostMortemPending)
    }
}

/// Result of an automated backup restore test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "restore_test_result", rename_all = "snake_case")]
pub enum RestoreTestResult {
    Passed,
    Failed,
    Partial,
}

/// Regulatory body for compliance notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "regulatory_body", rename_all = "snake_case")]
pub enum RegulatoryBody {
    Cbn,
    Sec,
    PartnerFi,
    Internal,
}

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

/// Business Impact Analysis entry — maps a service to its MTD and criticality.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct BiaEntry {
    pub id: Uuid,
    pub service_name: String,
    pub criticality: ServiceCriticality,
    /// Maximum Tolerable Downtime in seconds.
    pub mtd_seconds: i64,
    /// Recovery Point Objective in seconds (target data loss window).
    pub rpo_seconds: i64,
    /// Recovery Time Objective in seconds (target restoration time).
    pub rto_seconds: i64,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Immutable backup record stored in air-gapped environment.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: Uuid,
    /// S3 object key in the immutable backup bucket.
    pub s3_key: String,
    pub s3_bucket: String,
    /// SHA-256 checksum of the backup archive.
    pub checksum_sha256: String,
    /// Compressed size in bytes.
    pub size_bytes: i64,
    /// Whether this backup has been verified by a restore test.
    pub verified: bool,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Result of an automated restore verification pipeline run.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RestoreTestRun {
    pub id: Uuid,
    pub backup_id: Uuid,
    pub result: RestoreTestResult,
    /// Duration of the restore operation in seconds.
    pub restore_duration_seconds: i64,
    /// Actual data loss window observed (RPO achieved).
    pub rpo_achieved_seconds: Option<i64>,
    /// Time from trigger to full restoration (RTO achieved).
    pub rto_achieved_seconds: Option<i64>,
    pub error_message: Option<String>,
    pub run_at: DateTime<Utc>,
}

/// A declared DR incident.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DrIncident {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: DrIncidentStatus,
    /// Incident commander (ERT lead) user ID.
    pub commander_id: String,
    /// Affected services (JSON array of service names).
    pub affected_services: serde_json::Value,
    /// Actual RPO achieved at resolution (seconds).
    pub rpo_achieved_seconds: Option<i64>,
    /// Actual RTO achieved at resolution (seconds).
    pub rto_achieved_seconds: Option<i64>,
    pub declared_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Regulatory notification sent during a DR incident.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RegulatoryNotification {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub body: RegulatoryBody,
    pub template_used: String,
    pub sent_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DeclareDrIncidentRequest {
    pub title: String,
    pub description: String,
    pub commander_id: String,
    pub affected_services: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIncidentStatusRequest {
    pub status: DrIncidentStatus,
    pub rpo_achieved_seconds: Option<i64>,
    pub rto_achieved_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SendRegulatoryNotificationRequest {
    pub body: RegulatoryBody,
}

#[derive(Debug, Serialize)]
pub struct DrStatusResponse {
    pub active_incidents: Vec<DrIncident>,
    pub last_backup: Option<BackupRecord>,
    pub last_restore_test: Option<RestoreTestRun>,
    pub bia_entries: Vec<BiaEntry>,
}
