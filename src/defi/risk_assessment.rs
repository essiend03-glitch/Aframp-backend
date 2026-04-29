/// Task 1 — DeFi Risk Assessment & Protocol Health Monitoring
///
/// Implements a comprehensive risk assessment framework covering smart contract risk,
/// economic risk, operational risk, and concentration risk for all integrated DeFi protocols.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::DbPool;
use crate::error::AppError;

// ── Risk Category Weights ─────────────────────────────────────────────────────

/// Configurable weights for composite risk score calculation (must sum to 1.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCategoryWeights {
    pub smart_contract: f64,
    pub economic: f64,
    pub operational: f64,
    pub concentration: f64,
}

impl Default for RiskCategoryWeights {
    fn default() -> Self {
        Self {
            smart_contract: 0.35,
            economic: 0.30,
            operational: 0.20,
            concentration: 0.15,
        }
    }
}

// ── Risk Tier Classification ──────────────────────────────────────────────────

/// Protocol risk tier based on composite score (0–100)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "protocol_risk_tier", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ProtocolRiskTier {
    /// Score < 30 — eligible for full deployment
    Green,
    /// Score 30–60 — reduced limits, governance approval required above threshold
    Amber,
    /// Score > 60 — no new deployments
    Red,
}

impl ProtocolRiskTier {
    pub fn from_score(score: f64) -> Self {
        if score < 30.0 {
            Self::Green
        } else if score <= 60.0 {
            Self::Amber
        } else {
            Self::Red
        }
    }
}

// ── Smart Contract Risk ───────────────────────────────────────────────────────

/// Audit record for a DeFi protocol
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProtocolAuditRecord {
    pub audit_id: Uuid,
    pub protocol_id: String,
    pub audit_firm: String,
    pub audit_date: DateTime<Utc>,
    pub audit_scope: String,
    pub critical_findings: i32,
    pub high_findings: i32,
    pub medium_findings: i32,
    pub low_findings: i32,
    pub unresolved_critical: i32,
    pub remediation_status: AuditRemediationStatus,
    pub report_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "audit_remediation_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AuditRemediationStatus {
    FullyRemediated,
    PartiallyRemediated,
    Unresolved,
    Acknowledged,
}

/// Smart contract vulnerability disclosure
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VulnerabilityDisclosure {
    pub disclosure_id: Uuid,
    pub protocol_id: String,
    pub severity: VulnerabilitySeverity,
    pub title: String,
    pub description: String,
    pub cve_id: Option<String>,
    pub disclosed_at: DateTime<Utc>,
    pub patched_at: Option<DateTime<Utc>>,
    pub affects_platform_positions: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "vulnerability_severity", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum VulnerabilitySeverity {
    Critical,
    High,
    Medium,
    Low,
    Informational,
}

/// Unplanned smart contract upgrade detection record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UnplannedUpgradeRecord {
    pub upgrade_id: Uuid,
    pub protocol_id: String,
    pub detected_at: DateTime<Utc>,
    pub previous_implementation: String,
    pub new_implementation: String,
    pub was_announced: bool,
    pub announcement_url: Option<String>,
    pub risk_assessment: String,
    pub created_at: DateTime<Utc>,
}

/// Smart contract risk score component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartContractRiskScore {
    pub protocol_id: String,
    /// 0–100, higher = riskier
    pub score: f64,
    pub audit_recency_score: f64,
    pub audit_quality_score: f64,
    pub unresolved_critical_score: f64,
    pub unplanned_upgrade_score: f64,
    pub computed_at: DateTime<Utc>,
}

// ── Economic Risk ─────────────────────────────────────────────────────────────

/// Economic metrics snapshot for a protocol
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProtocolEconomicMetrics {
    pub metric_id: Uuid,
    pub protocol_id: String,
    pub tvl: BigDecimal,
    pub tvl_change_1h_pct: f64,
    pub tvl_change_24h_pct: f64,
    pub utilisation_rate: f64,
    pub oracle_price_deviation_pct: f64,
    pub oracle_last_updated_at: Option<DateTime<Utc>>,
    pub oracle_is_stale: bool,
    pub liquidation_rate_24h: f64,
    pub volume_24h: BigDecimal,
    pub recorded_at: DateTime<Utc>,
}

/// Economic risk score component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicRiskScore {
    pub protocol_id: String,
    /// 0–100, higher = riskier
    pub score: f64,
    pub tvl_stability_score: f64,
    pub utilisation_rate_score: f64,
    pub oracle_health_score: f64,
    pub liquidation_rate_score: f64,
    pub computed_at: DateTime<Utc>,
}

// ── Operational Risk ──────────────────────────────────────────────────────────

/// Governance proposal record for a protocol
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProtocolGovernanceProposal {
    pub proposal_id: Uuid,
    pub protocol_id: String,
    pub title: String,
    pub description: String,
    pub proposal_type: GovernanceProposalType,
    pub voting_start: DateTime<Utc>,
    pub voting_end: DateTime<Utc>,
    pub votes_for: BigDecimal,
    pub votes_against: BigDecimal,
    pub quorum_reached: bool,
    pub outcome: Option<ProposalOutcome>,
    pub material_impact_on_platform: bool,
    pub impact_assessment: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "governance_proposal_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum GovernanceProposalType {
    ParameterChange,
    FeeChange,
    AssetListing,
    AssetDelisting,
    UpgradeContract,
    EmergencyAction,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "proposal_outcome", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ProposalOutcome {
    Passed,
    Rejected,
    Cancelled,
    Pending,
}

/// Operational risk score component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalRiskScore {
    pub protocol_id: String,
    /// 0–100, higher = riskier
    pub score: f64,
    pub governance_activity_score: f64,
    pub team_risk_score: f64,
    pub regulatory_risk_score: f64,
    pub computed_at: DateTime<Utc>,
}

// ── Concentration Risk ────────────────────────────────────────────────────────

/// Platform-wide concentration metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentrationMetrics {
    pub computed_at: DateTime<Utc>,
    pub total_deployed: BigDecimal,
    pub protocol_concentrations: Vec<ProtocolConcentration>,
    pub asset_concentrations: Vec<AssetConcentration>,
    pub limit_breaches: Vec<ConcentrationLimitBreach>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConcentration {
    pub protocol_id: String,
    pub protocol_name: String,
    pub deployed_amount: BigDecimal,
    pub concentration_pct: f64,
    pub limit_pct: f64,
    pub is_breached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetConcentration {
    pub asset_code: String,
    pub deployed_amount: BigDecimal,
    pub concentration_pct: f64,
    pub limit_pct: f64,
    pub is_breached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentrationLimitBreach {
    pub breach_id: Uuid,
    pub dimension: String, // "protocol" | "asset"
    pub identifier: String,
    pub current_pct: f64,
    pub limit_pct: f64,
    pub detected_at: DateTime<Utc>,
}

/// Concentration risk score component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentrationRiskScore {
    pub score: f64,
    pub max_single_protocol_pct: f64,
    pub max_single_asset_pct: f64,
    pub computed_at: DateTime<Utc>,
}

// ── Composite Risk Score ──────────────────────────────────────────────────────

/// Full composite risk score for a protocol
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompositeRiskScore {
    pub score_id: Uuid,
    pub protocol_id: String,
    /// 0–100, higher = riskier
    pub composite_score: f64,
    pub smart_contract_score: f64,
    pub economic_score: f64,
    pub operational_score: f64,
    pub concentration_score: f64,
    pub risk_tier: ProtocolRiskTier,
    pub computed_at: DateTime<Utc>,
}

/// Historical risk score entry
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RiskScoreHistory {
    pub history_id: Uuid,
    pub protocol_id: String,
    pub composite_score: f64,
    pub risk_tier: ProtocolRiskTier,
    pub recorded_at: DateTime<Utc>,
}

// ── Stress Testing ────────────────────────────────────────────────────────────

/// Stress test scenario definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestScenario {
    pub scenario_id: Uuid,
    pub name: String,
    pub description: String,
    pub scenario_type: StressScenarioType,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StressScenarioType {
    TvlDropToZero,
    OracleManipulation,
    GovernanceAttack,
    RegulatoryShutdown,
    CorrelatedProtocolFailure,
    Custom,
}

/// Stress test result
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StressTestResult {
    pub result_id: Uuid,
    pub scenario_id: Uuid,
    pub scenario_name: String,
    pub estimated_loss: BigDecimal,
    pub estimated_loss_pct: f64,
    pub affected_protocols: Vec<String>,
    pub affected_positions: Vec<Uuid>,
    pub run_at: DateTime<Utc>,
    pub triggered_by: String,
}

// ── Risk Reports ──────────────────────────────────────────────────────────────

/// Generated risk report metadata
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RiskReport {
    pub report_id: Uuid,
    pub report_type: RiskReportType,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub generated_by: String,
    pub download_url: Option<String>,
    pub summary: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "risk_report_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RiskReportType {
    WeeklyGovernance,
    MonthlyManagement,
    AdHoc,
}

// ── Risk Assessment Service ───────────────────────────────────────────────────

pub struct RiskAssessmentService {
    db: Arc<DbPool>,
    weights: RiskCategoryWeights,
    /// Max single-protocol concentration % before breach
    max_protocol_concentration_pct: f64,
    /// Max single-asset concentration % before breach
    max_asset_concentration_pct: f64,
    /// TVL drop % within window that triggers alert
    tvl_drop_alert_threshold_pct: f64,
    /// Oracle deviation % that triggers alert
    oracle_deviation_alert_threshold_pct: f64,
}

impl RiskAssessmentService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self {
            db,
            weights: RiskCategoryWeights::default(),
            max_protocol_concentration_pct: 10.0,
            max_asset_concentration_pct: 25.0,
            tvl_drop_alert_threshold_pct: 20.0,
            oracle_deviation_alert_threshold_pct: 5.0,
        }
    }

    pub fn with_weights(mut self, weights: RiskCategoryWeights) -> Self {
        self.weights = weights;
        self
    }

    // ── Smart Contract Risk ───────────────────────────────────────────────────

    /// Compute smart contract risk score for a protocol (0–100)
    pub fn compute_smart_contract_score(
        &self,
        audits: &[ProtocolAuditRecord],
        upgrades: &[UnplannedUpgradeRecord],
    ) -> SmartContractRiskScore {
        let now = Utc::now();

        // Audit recency: penalise if most recent audit > 12 months old
        let recency_score = audits
            .iter()
            .map(|a| {
                let months_ago = (now - a.audit_date).num_days() as f64 / 30.0;
                (months_ago / 12.0 * 40.0).min(40.0)
            })
            .fold(40.0_f64, f64::min); // best (lowest risk) audit wins

        // Audit quality: penalise unresolved criticals
        let unresolved_critical_score = audits
            .iter()
            .map(|a| (a.unresolved_critical as f64 * 15.0).min(40.0))
            .fold(0.0_f64, f64::max);

        // Audit quality score (inverse of number of audits from reputable firms)
        let audit_quality_score = if audits.is_empty() {
            30.0
        } else {
            (30.0 / audits.len() as f64).min(30.0)
        };

        // Unplanned upgrades in last 90 days
        let recent_upgrades = upgrades
            .iter()
            .filter(|u| !u.was_announced && (now - u.detected_at).num_days() <= 90)
            .count();
        let upgrade_score = (recent_upgrades as f64 * 10.0).min(30.0);

        let score = (recency_score + unresolved_critical_score + audit_quality_score + upgrade_score)
            .clamp(0.0, 100.0);

        let protocol_id = audits
            .first()
            .map(|a| a.protocol_id.clone())
            .unwrap_or_default();

        SmartContractRiskScore {
            protocol_id,
            score,
            audit_recency_score: recency_score,
            audit_quality_score,
            unresolved_critical_score,
            unplanned_upgrade_score: upgrade_score,
            computed_at: now,
        }
    }

    // ── Economic Risk ─────────────────────────────────────────────────────────

    /// Compute economic risk score for a protocol (0–100)
    pub fn compute_economic_score(&self, metrics: &ProtocolEconomicMetrics) -> EconomicRiskScore {
        // TVL stability: penalise rapid outflows
        let tvl_stability_score = if metrics.tvl_change_24h_pct < -self.tvl_drop_alert_threshold_pct {
            40.0
        } else if metrics.tvl_change_24h_pct < -10.0 {
            20.0
        } else {
            0.0
        };

        // Utilisation rate: very high utilisation = liquidity risk
        let utilisation_score = if metrics.utilisation_rate > 0.95 {
            30.0
        } else if metrics.utilisation_rate > 0.80 {
            15.0
        } else {
            0.0
        };

        // Oracle health
        let oracle_score = if metrics.oracle_is_stale {
            20.0
        } else if metrics.oracle_price_deviation_pct.abs() > self.oracle_deviation_alert_threshold_pct {
            15.0
        } else {
            0.0
        };

        // Liquidation rate
        let liquidation_score = if metrics.liquidation_rate_24h > 0.10 {
            20.0
        } else if metrics.liquidation_rate_24h > 0.05 {
            10.0
        } else {
            0.0
        };

        let score = (tvl_stability_score + utilisation_score + oracle_score + liquidation_score)
            .clamp(0.0, 100.0);

        EconomicRiskScore {
            protocol_id: metrics.protocol_id.clone(),
            score,
            tvl_stability_score,
            utilisation_rate_score: utilisation_score,
            oracle_health_score: oracle_score,
            liquidation_rate_score: liquidation_score,
            computed_at: Utc::now(),
        }
    }

    // ── Operational Risk ──────────────────────────────────────────────────────

    /// Compute operational risk score (0–100)
    pub fn compute_operational_score(
        &self,
        proposals: &[ProtocolGovernanceProposal],
        team_anonymous: bool,
        regulatory_actions: u32,
    ) -> OperationalRiskScore {
        // Governance: recent material proposals that passed
        let material_passed = proposals
            .iter()
            .filter(|p| p.material_impact_on_platform && p.outcome == Some(ProposalOutcome::Passed))
            .count();
        let governance_score = (material_passed as f64 * 15.0).min(40.0);

        // Team risk
        let team_score = if team_anonymous { 30.0 } else { 10.0 };

        // Regulatory risk
        let regulatory_score = (regulatory_actions as f64 * 10.0).min(30.0);

        let score = (governance_score + team_score + regulatory_score).clamp(0.0, 100.0);

        OperationalRiskScore {
            protocol_id: String::new(), // caller sets this
            score,
            governance_activity_score: governance_score,
            team_risk_score: team_score,
            regulatory_risk_score: regulatory_score,
            computed_at: Utc::now(),
        }
    }

    // ── Concentration Risk ────────────────────────────────────────────────────

    /// Compute concentration risk score (0–100) from current concentration metrics
    pub fn compute_concentration_score(&self, metrics: &ConcentrationMetrics) -> ConcentrationRiskScore {
        let max_protocol_pct = metrics
            .protocol_concentrations
            .iter()
            .map(|p| p.concentration_pct)
            .fold(0.0_f64, f64::max);

        let max_asset_pct = metrics
            .asset_concentrations
            .iter()
            .map(|a| a.concentration_pct)
            .fold(0.0_f64, f64::max);

        let protocol_score = ((max_protocol_pct / self.max_protocol_concentration_pct) * 50.0).min(50.0);
        let asset_score = ((max_asset_pct / self.max_asset_concentration_pct) * 50.0).min(50.0);

        let score = (protocol_score + asset_score).clamp(0.0, 100.0);

        ConcentrationRiskScore {
            score,
            max_single_protocol_pct: max_protocol_pct,
            max_single_asset_pct: max_asset_pct,
            computed_at: Utc::now(),
        }
    }

    // ── Composite Score ───────────────────────────────────────────────────────

    /// Combine all category scores into a single composite risk score
    pub fn compute_composite_score(
        &self,
        protocol_id: &str,
        sc: &SmartContractRiskScore,
        eco: &EconomicRiskScore,
        ops: &OperationalRiskScore,
        conc: &ConcentrationRiskScore,
    ) -> CompositeRiskScore {
        let composite = sc.score * self.weights.smart_contract
            + eco.score * self.weights.economic
            + ops.score * self.weights.operational
            + conc.score * self.weights.concentration;

        let composite = composite.clamp(0.0, 100.0);
        let risk_tier = ProtocolRiskTier::from_score(composite);

        CompositeRiskScore {
            score_id: Uuid::new_v4(),
            protocol_id: protocol_id.to_string(),
            composite_score: composite,
            smart_contract_score: sc.score,
            economic_score: eco.score,
            operational_score: ops.score,
            concentration_score: conc.score,
            risk_tier,
            computed_at: Utc::now(),
        }
    }

    // ── Stress Testing ────────────────────────────────────────────────────────

    /// Estimate platform loss under a stress scenario
    pub fn run_stress_test(
        &self,
        scenario: &StressTestScenario,
        positions: &[(Uuid, String, BigDecimal)], // (position_id, protocol_id, value)
        affected_protocol_ids: &[String],
    ) -> StressTestResult {
        let affected_set: std::collections::HashSet<&str> =
            affected_protocol_ids.iter().map(|s| s.as_str()).collect();

        let (estimated_loss, affected_positions): (BigDecimal, Vec<Uuid>) = positions
            .iter()
            .filter(|(_, pid, _)| affected_set.contains(pid.as_str()))
            .fold(
                (BigDecimal::from(0), Vec::new()),
                |(acc_loss, mut acc_ids), (pos_id, _, value)| {
                    acc_ids.push(*pos_id);
                    (acc_loss + value.clone(), acc_ids)
                },
            );

        let total_value: BigDecimal = positions.iter().map(|(_, _, v)| v.clone()).sum();
        let loss_pct = if total_value > BigDecimal::from(0) {
            let ratio = estimated_loss.clone() / total_value;
            ratio.to_string().parse::<f64>().unwrap_or(0.0) * 100.0
        } else {
            0.0
        };

        StressTestResult {
            result_id: Uuid::new_v4(),
            scenario_id: scenario.scenario_id,
            scenario_name: scenario.name.clone(),
            estimated_loss,
            estimated_loss_pct: loss_pct,
            affected_protocols: affected_protocol_ids.to_vec(),
            affected_positions,
            run_at: Utc::now(),
            triggered_by: "system".to_string(),
        }
    }

    // ── Concentration Limit Checks ────────────────────────────────────────────

    /// Detect concentration limit breaches from current metrics
    pub fn detect_concentration_breaches(
        &self,
        metrics: &ConcentrationMetrics,
    ) -> Vec<ConcentrationLimitBreach> {
        let mut breaches = Vec::new();
        let now = Utc::now();

        for pc in &metrics.protocol_concentrations {
            if pc.concentration_pct > pc.limit_pct {
                breaches.push(ConcentrationLimitBreach {
                    breach_id: Uuid::new_v4(),
                    dimension: "protocol".to_string(),
                    identifier: pc.protocol_id.clone(),
                    current_pct: pc.concentration_pct,
                    limit_pct: pc.limit_pct,
                    detected_at: now,
                });
            }
        }

        for ac in &metrics.asset_concentrations {
            if ac.concentration_pct > ac.limit_pct {
                breaches.push(ConcentrationLimitBreach {
                    breach_id: Uuid::new_v4(),
                    dimension: "asset".to_string(),
                    identifier: ac.asset_code.clone(),
                    current_pct: ac.concentration_pct,
                    limit_pct: ac.limit_pct,
                    detected_at: now,
                });
            }
        }

        breaches
    }

    // ── TVL Drop Detection ────────────────────────────────────────────────────

    /// Returns true if TVL dropped beyond the configured threshold
    pub fn is_tvl_drop_alert(&self, tvl_change_pct: f64) -> bool {
        tvl_change_pct < -self.tvl_drop_alert_threshold_pct
    }

    /// Returns true if oracle deviation exceeds threshold
    pub fn is_oracle_deviation_alert(&self, deviation_pct: f64) -> bool {
        deviation_pct.abs() > self.oracle_deviation_alert_threshold_pct
    }
}

// ── HTTP Handlers ─────────────────────────────────────────────────────────────

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};

pub struct RiskAssessmentHandlers;

impl RiskAssessmentHandlers {
    /// GET /api/admin/defi/protocols/:protocol_id/audits
    pub async fn get_protocol_audits(
        State(svc): State<Arc<RiskAssessmentService>>,
        Path(protocol_id): Path<String>,
    ) -> Result<Json<Vec<ProtocolAuditRecord>>, AppError> {
        let records = sqlx::query_as::<_, ProtocolAuditRecord>(
            "SELECT * FROM defi_protocol_audits WHERE protocol_id = $1 ORDER BY audit_date DESC",
        )
        .bind(&protocol_id)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(records))
    }

    /// GET /api/admin/defi/protocols/:protocol_id/economic-metrics
    pub async fn get_economic_metrics(
        State(svc): State<Arc<RiskAssessmentService>>,
        Path(protocol_id): Path<String>,
        Query(params): Query<MetricsHistoryParams>,
    ) -> Result<Json<Vec<ProtocolEconomicMetrics>>, AppError> {
        let limit = params.limit.unwrap_or(100);
        let records = sqlx::query_as::<_, ProtocolEconomicMetrics>(
            "SELECT * FROM defi_economic_metrics WHERE protocol_id = $1 ORDER BY recorded_at DESC LIMIT $2",
        )
        .bind(&protocol_id)
        .bind(limit)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(records))
    }

    /// GET /api/admin/defi/protocols/:protocol_id/governance-activity
    pub async fn get_governance_activity(
        State(svc): State<Arc<RiskAssessmentService>>,
        Path(protocol_id): Path<String>,
    ) -> Result<Json<Vec<ProtocolGovernanceProposal>>, AppError> {
        let records = sqlx::query_as::<_, ProtocolGovernanceProposal>(
            "SELECT * FROM defi_governance_proposals WHERE protocol_id = $1 ORDER BY voting_start DESC",
        )
        .bind(&protocol_id)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(records))
    }

    /// GET /api/admin/defi/risk/concentration
    pub async fn get_concentration_metrics(
        State(svc): State<Arc<RiskAssessmentService>>,
    ) -> Result<Json<ConcentrationMetrics>, AppError> {
        // Fetch live concentration data from DB and compute
        let protocol_rows = sqlx::query!(
            r#"
            SELECT protocol_id, protocol_name,
                   SUM(current_value) AS deployed_amount,
                   SUM(current_value) / NULLIF(SUM(SUM(current_value)) OVER (), 0) * 100 AS concentration_pct
            FROM defi_positions
            WHERE position_status = 'active'
            GROUP BY protocol_id, protocol_name
            "#
        )
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        let protocol_concentrations: Vec<ProtocolConcentration> = protocol_rows
            .into_iter()
            .map(|r| ProtocolConcentration {
                protocol_id: r.protocol_id.clone(),
                protocol_name: r.protocol_name.unwrap_or_default(),
                deployed_amount: r.deployed_amount.unwrap_or_default(),
                concentration_pct: r.concentration_pct.unwrap_or(0.0),
                limit_pct: svc.max_protocol_concentration_pct,
                is_breached: r.concentration_pct.unwrap_or(0.0) > svc.max_protocol_concentration_pct,
            })
            .collect();

        let total_deployed: BigDecimal = protocol_concentrations
            .iter()
            .map(|p| p.deployed_amount.clone())
            .sum();

        let metrics = ConcentrationMetrics {
            computed_at: Utc::now(),
            total_deployed,
            protocol_concentrations: protocol_concentrations.clone(),
            asset_concentrations: vec![],
            limit_breaches: svc.detect_concentration_breaches(&ConcentrationMetrics {
                computed_at: Utc::now(),
                total_deployed: BigDecimal::from(0),
                protocol_concentrations: protocol_concentrations.clone(),
                asset_concentrations: vec![],
                limit_breaches: vec![],
            }),
        };

        Ok(Json(metrics))
    }

    /// GET /api/admin/defi/protocols/:protocol_id/risk-score
    pub async fn get_protocol_risk_score(
        State(svc): State<Arc<RiskAssessmentService>>,
        Path(protocol_id): Path<String>,
    ) -> Result<Json<CompositeRiskScoreResponse>, AppError> {
        let current = sqlx::query_as::<_, CompositeRiskScore>(
            "SELECT * FROM defi_composite_risk_scores WHERE protocol_id = $1 ORDER BY computed_at DESC LIMIT 1",
        )
        .bind(&protocol_id)
        .fetch_optional(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        let history = sqlx::query_as::<_, RiskScoreHistory>(
            "SELECT * FROM defi_risk_score_history WHERE protocol_id = $1 ORDER BY recorded_at DESC LIMIT 90",
        )
        .bind(&protocol_id)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        Ok(Json(CompositeRiskScoreResponse { current, history }))
    }

    /// GET /api/admin/defi/risk/overview
    pub async fn get_risk_overview(
        State(svc): State<Arc<RiskAssessmentService>>,
    ) -> Result<Json<Vec<CompositeRiskScore>>, AppError> {
        let scores = sqlx::query_as::<_, CompositeRiskScore>(
            r#"
            SELECT DISTINCT ON (protocol_id) *
            FROM defi_composite_risk_scores
            ORDER BY protocol_id, computed_at DESC
            "#,
        )
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(scores))
    }

    /// POST /api/admin/defi/risk/stress-test
    pub async fn run_stress_test(
        State(svc): State<Arc<RiskAssessmentService>>,
        Json(req): Json<RunStressTestRequest>,
    ) -> Result<Json<StressTestResult>, AppError> {
        let positions = sqlx::query!(
            "SELECT position_id, protocol_id, current_value FROM defi_positions WHERE position_status = 'active'"
        )
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        let pos_tuples: Vec<(Uuid, String, BigDecimal)> = positions
            .into_iter()
            .map(|r| (r.position_id, r.protocol_id, r.current_value))
            .collect();

        let result = svc.run_stress_test(&req.scenario, &pos_tuples, &req.affected_protocol_ids);

        sqlx::query!(
            r#"INSERT INTO defi_stress_test_results
               (result_id, scenario_id, scenario_name, estimated_loss, estimated_loss_pct,
                affected_protocols, affected_positions, run_at, triggered_by)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            result.result_id,
            result.scenario_id,
            &result.scenario_name,
            result.estimated_loss,
            result.estimated_loss_pct,
            &result.affected_protocols as &[String],
            &result.affected_positions as &[Uuid],
            result.run_at,
            &result.triggered_by,
        )
        .execute(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        Ok(Json(result))
    }

    /// GET /api/admin/defi/risk/stress-test/results
    pub async fn list_stress_test_results(
        State(svc): State<Arc<RiskAssessmentService>>,
        Query(params): Query<MetricsHistoryParams>,
    ) -> Result<Json<Vec<StressTestResult>>, AppError> {
        let limit = params.limit.unwrap_or(50);
        let results = sqlx::query_as::<_, StressTestResult>(
            "SELECT * FROM defi_stress_test_results ORDER BY run_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(results))
    }

    /// GET /api/admin/defi/risk/reports
    pub async fn list_risk_reports(
        State(svc): State<Arc<RiskAssessmentService>>,
    ) -> Result<Json<Vec<RiskReport>>, AppError> {
        let reports = sqlx::query_as::<_, RiskReport>(
            "SELECT * FROM defi_risk_reports ORDER BY generated_at DESC",
        )
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(reports))
    }
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MetricsHistoryParams {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RunStressTestRequest {
    pub scenario: StressTestScenario,
    pub affected_protocol_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CompositeRiskScoreResponse {
    pub current: Option<CompositeRiskScore>,
    pub history: Vec<RiskScoreHistory>,
}

// ── Routes ────────────────────────────────────────────────────────────────────

pub fn risk_assessment_routes(svc: Arc<RiskAssessmentService>) -> Router {
    Router::new()
        .route("/protocols/:protocol_id/audits", get(RiskAssessmentHandlers::get_protocol_audits))
        .route("/protocols/:protocol_id/economic-metrics", get(RiskAssessmentHandlers::get_economic_metrics))
        .route("/protocols/:protocol_id/governance-activity", get(RiskAssessmentHandlers::get_governance_activity))
        .route("/protocols/:protocol_id/risk-score", get(RiskAssessmentHandlers::get_protocol_risk_score))
        .route("/risk/concentration", get(RiskAssessmentHandlers::get_concentration_metrics))
        .route("/risk/overview", get(RiskAssessmentHandlers::get_risk_overview))
        .route("/risk/stress-test", post(RiskAssessmentHandlers::run_stress_test))
        .route("/risk/stress-test/results", get(RiskAssessmentHandlers::list_stress_test_results))
        .route("/risk/reports", get(RiskAssessmentHandlers::list_risk_reports))
        .with_state(svc)
}

// ── Observability ─────────────────────────────────────────────────────────────

/// Emit Prometheus metrics and structured logs for risk events
pub fn record_risk_tier_transition(
    protocol_id: &str,
    from_tier: &ProtocolRiskTier,
    to_tier: &ProtocolRiskTier,
    composite_score: f64,
) {
    tracing::warn!(
        protocol_id = %protocol_id,
        from_tier = ?from_tier,
        to_tier = ?to_tier,
        composite_score = composite_score,
        "DeFi protocol risk tier transition"
    );
}

pub fn record_concentration_breach(breach: &ConcentrationLimitBreach) {
    tracing::warn!(
        dimension = %breach.dimension,
        identifier = %breach.identifier,
        current_pct = breach.current_pct,
        limit_pct = breach.limit_pct,
        "DeFi concentration limit breached"
    );
}

pub fn record_critical_vulnerability(disclosure: &VulnerabilityDisclosure) {
    tracing::error!(
        protocol_id = %disclosure.protocol_id,
        severity = ?disclosure.severity,
        title = %disclosure.title,
        affects_platform = disclosure.affects_platform_positions,
        "Critical DeFi vulnerability disclosure detected"
    );
}

pub fn record_unplanned_upgrade(upgrade: &UnplannedUpgradeRecord) {
    tracing::error!(
        protocol_id = %upgrade.protocol_id,
        previous_impl = %upgrade.previous_implementation,
        new_impl = %upgrade.new_implementation,
        "Unplanned smart contract upgrade detected"
    );
}

// ── Unit Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_service() -> RiskAssessmentService {
        // Use a dummy Arc<DbPool> — unit tests don't hit the DB
        use std::sync::Arc;
        // We can't construct a real pool in unit tests, so we test pure logic only
        // by calling methods that don't require DB access.
        RiskAssessmentService {
            db: unsafe { Arc::from_raw(std::ptr::NonNull::dangling().as_ptr()) },
            weights: RiskCategoryWeights::default(),
            max_protocol_concentration_pct: 10.0,
            max_asset_concentration_pct: 25.0,
            tvl_drop_alert_threshold_pct: 20.0,
            oracle_deviation_alert_threshold_pct: 5.0,
        }
    }

    #[test]
    fn test_risk_tier_from_score() {
        assert_eq!(ProtocolRiskTier::from_score(0.0), ProtocolRiskTier::Green);
        assert_eq!(ProtocolRiskTier::from_score(29.9), ProtocolRiskTier::Green);
        assert_eq!(ProtocolRiskTier::from_score(30.0), ProtocolRiskTier::Amber);
        assert_eq!(ProtocolRiskTier::from_score(60.0), ProtocolRiskTier::Amber);
        assert_eq!(ProtocolRiskTier::from_score(60.1), ProtocolRiskTier::Red);
        assert_eq!(ProtocolRiskTier::from_score(100.0), ProtocolRiskTier::Red);
    }

    #[test]
    fn test_composite_score_weighted_calculation() {
        let svc = RiskAssessmentService {
            db: unsafe { Arc::from_raw(std::ptr::NonNull::dangling().as_ptr()) },
            weights: RiskCategoryWeights {
                smart_contract: 0.35,
                economic: 0.30,
                operational: 0.20,
                concentration: 0.15,
            },
            max_protocol_concentration_pct: 10.0,
            max_asset_concentration_pct: 25.0,
            tvl_drop_alert_threshold_pct: 20.0,
            oracle_deviation_alert_threshold_pct: 5.0,
        };

        let sc = SmartContractRiskScore {
            protocol_id: "test".into(),
            score: 20.0,
            audit_recency_score: 5.0,
            audit_quality_score: 5.0,
            unresolved_critical_score: 5.0,
            unplanned_upgrade_score: 5.0,
            computed_at: Utc::now(),
        };
        let eco = EconomicRiskScore {
            protocol_id: "test".into(),
            score: 40.0,
            tvl_stability_score: 10.0,
            utilisation_rate_score: 10.0,
            oracle_health_score: 10.0,
            liquidation_rate_score: 10.0,
            computed_at: Utc::now(),
        };
        let ops = OperationalRiskScore {
            protocol_id: "test".into(),
            score: 50.0,
            governance_activity_score: 20.0,
            team_risk_score: 20.0,
            regulatory_risk_score: 10.0,
            computed_at: Utc::now(),
        };
        let conc = ConcentrationRiskScore {
            score: 10.0,
            max_single_protocol_pct: 5.0,
            max_single_asset_pct: 10.0,
            computed_at: Utc::now(),
        };

        let result = svc.compute_composite_score("test", &sc, &eco, &ops, &conc);
        // 20*0.35 + 40*0.30 + 50*0.20 + 10*0.15 = 7 + 12 + 10 + 1.5 = 30.5
        assert!((result.composite_score - 30.5).abs() < 0.01);
        assert_eq!(result.risk_tier, ProtocolRiskTier::Amber);
    }

    #[test]
    fn test_tvl_drop_alert() {
        let svc = make_service();
        assert!(svc.is_tvl_drop_alert(-25.0));
        assert!(!svc.is_tvl_drop_alert(-10.0));
        assert!(!svc.is_tvl_drop_alert(5.0));
    }

    #[test]
    fn test_oracle_deviation_alert() {
        let svc = make_service();
        assert!(svc.is_oracle_deviation_alert(6.0));
        assert!(svc.is_oracle_deviation_alert(-6.0));
        assert!(!svc.is_oracle_deviation_alert(3.0));
    }

    #[test]
    fn test_concentration_breach_detection() {
        let svc = make_service();
        let metrics = ConcentrationMetrics {
            computed_at: Utc::now(),
            total_deployed: BigDecimal::from(1000),
            protocol_concentrations: vec![
                ProtocolConcentration {
                    protocol_id: "proto_a".into(),
                    protocol_name: "Protocol A".into(),
                    deployed_amount: BigDecimal::from(150),
                    concentration_pct: 15.0, // exceeds 10% limit
                    limit_pct: 10.0,
                    is_breached: true,
                },
                ProtocolConcentration {
                    protocol_id: "proto_b".into(),
                    protocol_name: "Protocol B".into(),
                    deployed_amount: BigDecimal::from(80),
                    concentration_pct: 8.0,
                    limit_pct: 10.0,
                    is_breached: false,
                },
            ],
            asset_concentrations: vec![],
            limit_breaches: vec![],
        };

        let breaches = svc.detect_concentration_breaches(&metrics);
        assert_eq!(breaches.len(), 1);
        assert_eq!(breaches[0].identifier, "proto_a");
    }

    #[test]
    fn test_stress_test_loss_estimation() {
        let svc = make_service();
        let scenario = StressTestScenario {
            scenario_id: Uuid::new_v4(),
            name: "TVL Drop to Zero".into(),
            description: "Protocol A loses all TVL".into(),
            scenario_type: StressScenarioType::TvlDropToZero,
            parameters: serde_json::json!({}),
        };

        let pos_a = Uuid::new_v4();
        let pos_b = Uuid::new_v4();
        let positions = vec![
            (pos_a, "proto_a".to_string(), BigDecimal::from(500)),
            (pos_b, "proto_b".to_string(), BigDecimal::from(500)),
        ];

        let result = svc.run_stress_test(&scenario, &positions, &["proto_a".to_string()]);
        assert_eq!(result.estimated_loss, BigDecimal::from(500));
        assert!((result.estimated_loss_pct - 50.0).abs() < 0.01);
        assert_eq!(result.affected_positions.len(), 1);
        assert_eq!(result.affected_positions[0], pos_a);
    }

    #[test]
    fn test_smart_contract_score_no_audits() {
        let svc = make_service();
        let score = svc.compute_smart_contract_score(&[], &[]);
        // No audits = high risk
        assert!(score.score > 0.0);
    }

    #[test]
    fn test_economic_score_stale_oracle() {
        let svc = make_service();
        let metrics = ProtocolEconomicMetrics {
            metric_id: Uuid::new_v4(),
            protocol_id: "test".into(),
            tvl: BigDecimal::from(1_000_000),
            tvl_change_1h_pct: 0.0,
            tvl_change_24h_pct: -5.0,
            utilisation_rate: 0.5,
            oracle_price_deviation_pct: 0.0,
            oracle_last_updated_at: None,
            oracle_is_stale: true,
            liquidation_rate_24h: 0.01,
            volume_24h: BigDecimal::from(100_000),
            recorded_at: Utc::now(),
        };
        let score = svc.compute_economic_score(&metrics);
        assert!(score.oracle_health_score > 0.0);
    }
}
