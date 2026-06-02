use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Evidence Package ──────────────────────────────────────────────────────────

/// A complete regulatory evidence package for a given scope/period.
#[derive(Debug, Clone, Serialize)]
pub struct EvidencePackage {
    pub id: Uuid,
    pub scope_label: String,
    pub period_from: DateTime<Utc>,
    pub period_to: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub generated_by: String,
    /// SHA-256 of the serialised payload
    pub checksum_sha256: String,
    /// HMAC-SHA256 signature (hex) — proves platform origin
    pub signature_hmac_sha256: String,
    pub aml_log_count: i64,
    pub travel_rule_count: i64,
    pub kyc_event_count: i64,
    pub multisig_event_count: i64,
    pub policy_snapshot_count: i64,
    pub system_test_count: i64,
}

/// Stored evidence package record (DB row).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EvidencePackageRecord {
    pub id: Uuid,
    pub scope_label: String,
    pub period_from: DateTime<Utc>,
    pub period_to: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub generated_by: String,
    pub checksum_sha256: String,
    pub signature_hmac_sha256: String,
    pub aml_log_count: i64,
    pub travel_rule_count: i64,
    pub kyc_event_count: i64,
    pub multisig_event_count: i64,
    pub policy_snapshot_count: i64,
    pub system_test_count: i64,
}

// ── Policy History ────────────────────────────────────────────────────────────

/// A point-in-time snapshot of a compliance policy.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PolicySnapshot {
    pub id: Uuid,
    pub policy_name: String,
    pub policy_version: String,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    /// Full policy state as JSON (thresholds, rules, etc.)
    pub policy_state: serde_json::Value,
    pub changed_by: String,
    pub change_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicySnapshotRequest {
    pub policy_name: String,
    pub policy_version: String,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    pub policy_state: serde_json::Value,
    pub changed_by: String,
    pub change_reason: Option<String>,
}

// ── System Test Report ────────────────────────────────────────────────────────

/// A system test/health report attached to evidence packages.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SystemTestReport {
    pub id: Uuid,
    pub report_type: String, // "aml_stress_test" | "pentest" | "security_scan" | "dr_test"
    pub report_label: String,
    pub executed_at: DateTime<Utc>,
    pub executed_by: String,
    pub outcome: String, // "pass" | "fail" | "partial"
    pub summary: String,
    pub findings: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSystemTestReportRequest {
    pub report_type: String,
    pub report_label: String,
    pub executed_at: DateTime<Utc>,
    pub executed_by: String,
    pub outcome: String,
    pub summary: String,
    pub findings: serde_json::Value,
}

// ── Request / Response ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GenerateEvidencePackageRequest {
    pub scope_label: String,
    pub period_from: DateTime<Utc>,
    pub period_to: DateTime<Utc>,
    pub generated_by: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PolicyAtPointInTimeQuery {
    pub policy_name: String,
    pub at_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct EvidencePackageListQuery {
    pub scope_label: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
