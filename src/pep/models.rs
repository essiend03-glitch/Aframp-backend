//! PEP data models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Influence level of the PEP — drives the base risk score
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum PepInfluenceLevel {
    /// Head of State, Head of Government, senior cabinet minister
    HeadOfState,
    /// National-level minister, senior legislator, senior judiciary
    NationalSenior,
    /// Regional / state-level official, military officer (colonel+)
    RegionalSenior,
    /// Local / municipal official, junior military officer
    LocalOfficial,
    /// Executive of a state-owned enterprise
    StateEnterpriseExec,
}

impl PepInfluenceLevel {
    /// Base risk weight (0.0–1.0) for this influence level
    pub fn base_weight(&self) -> f64 {
        match self {
            PepInfluenceLevel::HeadOfState => 1.0,
            PepInfluenceLevel::NationalSenior => 0.85,
            PepInfluenceLevel::RegionalSenior => 0.65,
            PepInfluenceLevel::LocalOfficial => 0.40,
            PepInfluenceLevel::StateEnterpriseExec => 0.70,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PepInfluenceLevel::HeadOfState => "HEAD_OF_STATE",
            PepInfluenceLevel::NationalSenior => "NATIONAL_SENIOR",
            PepInfluenceLevel::RegionalSenior => "REGIONAL_SENIOR",
            PepInfluenceLevel::LocalOfficial => "LOCAL_OFFICIAL",
            PepInfluenceLevel::StateEnterpriseExec => "STATE_ENTERPRISE_EXEC",
        }
    }
}

/// Relationship of the matched individual to the PEP
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum PepRelationshipType {
    /// The individual IS the PEP
    DirectPep,
    /// Immediate family member (spouse, child, parent, sibling)
    ImmediateFamily,
    /// Close associate (business partner, known associate)
    CloseAssociate,
}

impl PepRelationshipType {
    /// Multiplier applied to the influence-level base weight
    pub fn weight_multiplier(&self) -> f64 {
        match self {
            PepRelationshipType::DirectPep => 1.0,
            PepRelationshipType::ImmediateFamily => 0.75,
            PepRelationshipType::CloseAssociate => 0.55,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PepRelationshipType::DirectPep => "DIRECT_PEP",
            PepRelationshipType::ImmediateFamily => "IMMEDIATE_FAMILY",
            PepRelationshipType::CloseAssociate => "CLOSE_ASSOCIATE",
        }
    }
}

/// Final risk tier after scoring
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum PepRiskTier {
    /// Score < 0.30 — no EDD required, log only
    Low,
    /// Score 0.30–0.59 — enhanced monitoring, periodic review
    Medium,
    /// Score 0.60–0.79 — EDD required, compliance officer review
    High,
    /// Score ≥ 0.80 — EDD + senior management sign-off before account approval
    Critical,
}

impl PepRiskTier {
    pub fn from_score(score: f64) -> Self {
        if score >= 0.80 {
            PepRiskTier::Critical
        } else if score >= 0.60 {
            PepRiskTier::High
        } else if score >= 0.30 {
            PepRiskTier::Medium
        } else {
            PepRiskTier::Low
        }
    }

    pub fn requires_edd(&self) -> bool {
        matches!(self, PepRiskTier::High | PepRiskTier::Critical)
    }

    pub fn requires_senior_signoff(&self) -> bool {
        matches!(self, PepRiskTier::Critical)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PepRiskTier::Low => "LOW",
            PepRiskTier::Medium => "MEDIUM",
            PepRiskTier::High => "HIGH",
            PepRiskTier::Critical => "CRITICAL",
        }
    }
}

/// Lifecycle status of a PEP match record
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum PepMatchStatus {
    /// Awaiting compliance review
    PendingReview,
    /// Confirmed as a true PEP match by compliance
    Confirmed,
    /// Dismissed as a false positive by compliance
    FalsePositive,
    /// Automatically suppressed by contextual filtering
    AutoSuppressed,
}

/// A single PEP match returned by the screening engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepMatch {
    pub match_id: Uuid,
    pub consumer_id: Uuid,
    pub matched_name: String,
    pub matched_aliases: Vec<String>,
    /// Fuzzy match score 0–100
    pub match_score: u8,
    pub influence_level: PepInfluenceLevel,
    pub relationship_type: PepRelationshipType,
    /// ISO 3166-1 alpha-2 country of the PEP's jurisdiction
    pub jurisdiction: String,
    /// Corruption Perception Index score for the jurisdiction (0–100, higher = cleaner)
    pub cpi_score: u8,
    /// Composite risk score 0.0–1.0
    pub risk_score: f64,
    pub risk_tier: PepRiskTier,
    pub status: PepMatchStatus,
    /// External provider's entity ID for this PEP record
    pub provider_entity_id: Option<String>,
    pub screened_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<Uuid>,
    pub review_notes: Option<String>,
}

/// Input to the PEP screening pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepScreeningRequest {
    pub consumer_id: Uuid,
    pub full_name: String,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub nationality: Option<String>,
    /// ISO 3166-1 alpha-2 country of residence
    pub country_of_residence: Option<String>,
    /// Whether this is an initial onboarding screen or a periodic re-screen
    pub is_rescreening: bool,
}

/// Result of a PEP screening run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepScreeningResult {
    pub consumer_id: Uuid,
    pub matches: Vec<PepMatch>,
    /// Highest risk tier across all matches (None if no matches)
    pub highest_risk_tier: Option<PepRiskTier>,
    /// Whether an EDD case was automatically created
    pub edd_triggered: bool,
    pub edd_case_id: Option<Uuid>,
    pub screened_at: DateTime<Utc>,
}

/// EDD case created for a confirmed high-risk PEP match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepEddCase {
    pub case_id: Uuid,
    pub consumer_id: Uuid,
    pub match_id: Uuid,
    pub risk_tier: PepRiskTier,
    pub status: PepEddStatus,
    /// Source of Wealth documentation notes
    pub source_of_wealth_notes: Option<String>,
    /// Source of Funds documentation notes
    pub source_of_funds_notes: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub requires_senior_signoff: bool,
    pub senior_signoff_by: Option<Uuid>,
    pub senior_signoff_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// EDD case lifecycle status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum PepEddStatus {
    /// Awaiting assignment to compliance officer
    Open,
    /// Assigned and under investigation
    InProgress,
    /// Pending senior management sign-off
    PendingSignoff,
    /// Approved — account may proceed
    Approved,
    /// Rejected — account blocked
    Rejected,
}

/// Tamper-proof audit log entry for every PEP screening event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepAuditEntry {
    pub entry_id: Uuid,
    pub consumer_id: Uuid,
    pub action: PepAuditAction,
    pub actor_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
    /// SHA-256 hash of (previous_hash || entry content) for chain integrity
    pub chain_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PepAuditAction {
    ScreeningPerformed,
    MatchConfirmed,
    MatchDismissed,
    AutoSuppressed,
    EddCaseCreated,
    EddCaseUpdated,
    SeniorSignoffGranted,
    SeniorSignoffDenied,
    RescreeningScheduled,
    StatusChanged,
}

impl std::fmt::Display for PepAuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PepAuditAction::ScreeningPerformed => "SCREENING_PERFORMED",
            PepAuditAction::MatchConfirmed => "MATCH_CONFIRMED",
            PepAuditAction::MatchDismissed => "MATCH_DISMISSED",
            PepAuditAction::AutoSuppressed => "AUTO_SUPPRESSED",
            PepAuditAction::EddCaseCreated => "EDD_CASE_CREATED",
            PepAuditAction::EddCaseUpdated => "EDD_CASE_UPDATED",
            PepAuditAction::SeniorSignoffGranted => "SENIOR_SIGNOFF_GRANTED",
            PepAuditAction::SeniorSignoffDenied => "SENIOR_SIGNOFF_DENIED",
            PepAuditAction::RescreeningScheduled => "RESCREENING_SCHEDULED",
            PepAuditAction::StatusChanged => "STATUS_CHANGED",
        };
        write!(f, "{}", s)
    }
}

/// Configuration for the PEP screening engine
#[derive(Debug, Clone)]
pub struct PepScreeningConfig {
    /// External PEP provider base URL (e.g. Dow Jones, Refinitiv)
    pub provider_base_url: String,
    pub provider_api_key: String,
    /// Minimum fuzzy match score (0–100) to surface a match
    pub match_threshold: u8,
    /// Matches below this score are auto-suppressed without manual review
    pub auto_suppress_threshold: u8,
    /// Cache TTL for negative (no-hit) results in seconds
    pub negative_cache_ttl_secs: u64,
    /// Fuzziness factor sent to the provider (0.0–1.0)
    pub fuzziness: f64,
}

impl Default for PepScreeningConfig {
    fn default() -> Self {
        Self {
            provider_base_url: "https://api.dowjones.com/risk-and-compliance/v1".into(),
            provider_api_key: String::new(),
            match_threshold: 70,
            auto_suppress_threshold: 50,
            negative_cache_ttl_secs: 3600,
            fuzziness: 0.7,
        }
    }
}
