use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use uuid::Uuid;

// ── SLO Definition ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SloDefinition {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub metric_name: String,
    pub operator: String,   // "lt" | "lte" | "gt" | "gte"
    pub threshold: BigDecimal,
    pub window_seconds: i32,
    pub severity: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Breach Incident ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SlaBreachIncident {
    pub id: Uuid,
    pub slo_id: Uuid,
    pub detected_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub status: String,
    pub observed_value: BigDecimal,
    pub threshold_value: BigDecimal,
    pub affected_service: String,
    pub root_cause_summary: Option<String>,
    pub remediation_steps: Option<String>,
    pub context_snapshot: serde_json::Value,
    pub partners_notified: bool,
    pub notification_sent_at: Option<DateTime<Utc>>,
    pub etr: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Post-Mortem ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SlaPostMortem {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub author: String,
    pub timeline: serde_json::Value,
    pub root_cause: String,
    pub contributing_factors: Option<String>,
    pub remediation: String,
    pub preventive_measures: String,
    pub action_items: serde_json::Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Compliance Report ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SlaComplianceReport {
    pub id: Uuid,
    pub partner_id: Option<Uuid>,
    pub report_month: chrono::NaiveDate,
    pub total_breaches: i32,
    pub mttr_seconds: Option<BigDecimal>,
    pub availability_pct: Option<BigDecimal>,
    pub breach_ids: Vec<Uuid>,
    pub generated_at: DateTime<Utc>,
}

// ── Request / Response DTOs ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpdateIncidentRequest {
    pub status: Option<String>,
    pub root_cause_summary: Option<String>,
    pub remediation_steps: Option<String>,
    pub etr: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePostMortemRequest {
    pub author: String,
    pub timeline: serde_json::Value,
    pub root_cause: String,
    pub contributing_factors: Option<String>,
    pub remediation: String,
    pub preventive_measures: String,
    pub action_items: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SlaComplianceDashboard {
    pub open_incidents: Vec<SlaBreachIncident>,
    pub slo_definitions: Vec<SloDefinition>,
    pub recent_breaches_30d: i64,
    pub mttr_seconds_30d: Option<f64>,
    pub availability_pct_30d: Option<f64>,
}
