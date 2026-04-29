/// Task 2 — DeFi Position Monitoring & Automated Rebalancing
///
/// Continuous monitoring of all active DeFi positions with adaptive frequency,
/// drift detection, impermanent loss tracking, and automated rebalancing execution.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::DbPool;
use crate::error::AppError;

// ── Monitoring Alert Hierarchy ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "monitoring_alert_level", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MonitoringAlertLevel {
    /// Drift within tolerance — no action needed
    Informational,
    /// Drift approaching tolerance — watch closely
    Warning,
    /// Drift exceeding tolerance or protocol health degrading
    Critical,
    /// Circuit breaker conditions met — emergency action required
    Emergency,
}

// ── Position Snapshot ─────────────────────────────────────────────────────────

/// Snapshot of a position captured at each monitoring cycle
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PositionSnapshot {
    pub snapshot_id: Uuid,
    pub position_id: Uuid,
    pub protocol_id: String,
    pub strategy_id: Option<Uuid>,
    /// Current value in deposited asset units
    pub current_value: BigDecimal,
    /// Current value in fiat equivalent (USD)
    pub current_value_fiat: BigDecimal,
    /// Absolute change since last snapshot
    pub value_change_abs: BigDecimal,
    /// Percentage change since last snapshot
    pub value_change_pct: f64,
    /// Cumulative change since position opened
    pub cumulative_change_pct: f64,
    /// Allocation drift from strategy target (percentage points)
    pub allocation_drift_pct: f64,
    /// Impermanent loss percentage (AMM positions only)
    pub impermanent_loss_pct: f64,
    /// Accrued fees since last snapshot
    pub accrued_fees: BigDecimal,
    /// Protocol health score at snapshot time
    pub protocol_health_score: f64,
    pub alert_level: MonitoringAlertLevel,
    pub snapshotted_at: DateTime<Utc>,
}

// ── Drift Status ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    WithinTolerance,
    ApproachingTolerance,
    ExceedingTolerance,
    SevereDrift,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolDriftStatus {
    pub protocol_id: String,
    pub target_allocation_pct: f64,
    pub current_allocation_pct: f64,
    pub drift_magnitude_pct: f64,
    pub severity: DriftSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDriftStatus {
    pub strategy_id: Uuid,
    pub computed_at: DateTime<Utc>,
    pub protocol_drifts: Vec<ProtocolDriftStatus>,
    pub requires_rebalancing: bool,
}

// ── Rebalancing ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalancingOperation {
    pub protocol_id: String,
    pub operation_type: RebalancingOperationType,
    pub amount: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RebalancingOperationType {
    Withdraw,
    Deposit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalancingPlan {
    pub plan_id: Uuid,
    pub strategy_id: Uuid,
    pub trigger_reason: String,
    pub operations: Vec<RebalancingOperation>,
    pub requires_governance_approval: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebalancingTriggerType {
    DriftBased,
    TimeBasedSchedule,
    ImpermanentLoss,
    ProtocolHealthDegradation,
    Emergency,
    Manual,
}

/// Persisted rebalancing audit event
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RebalancingAuditEvent {
    pub event_id: Uuid,
    pub strategy_id: Uuid,
    pub trigger_type: String,
    pub trigger_reason: String,
    pub pre_rebalancing_state: serde_json::Value,
    pub executed_operations: serde_json::Value,
    pub post_rebalancing_state: serde_json::Value,
    pub outcome: String,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ── Monitoring Alert ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MonitoringAlert {
    pub alert_id: Uuid,
    pub alert_level: MonitoringAlertLevel,
    pub position_id: Option<Uuid>,
    pub protocol_id: Option<String>,
    pub strategy_id: Option<Uuid>,
    pub message: String,
    pub recommended_action: String,
    pub acknowledged_by: Option<String>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Protocol Health History ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProtocolHealthHistory {
    pub history_id: Uuid,
    pub protocol_id: String,
    pub health_score: f64,
    pub tvl: BigDecimal,
    pub volume_24h: BigDecimal,
    pub active_users: i64,
    pub recorded_at: DateTime<Utc>,
}

// ── Impermanent Loss ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpermanentLossRecord {
    pub position_id: Uuid,
    pub pool_id: String,
    pub current_il_pct: f64,
    pub cumulative_il_pct: f64,
    pub deposit_price_ratio: f64,
    pub current_price_ratio: f64,
    pub computed_at: DateTime<Utc>,
}

// ── Position Monitoring Service ───────────────────────────────────────────────

pub struct PositionMonitoringService {
    db: Arc<DbPool>,
    /// Drift tolerance before informational alert (percentage points)
    drift_tolerance_pct: f64,
    /// Drift approaching tolerance threshold
    drift_warning_pct: f64,
    /// Drift exceeding tolerance — triggers rebalancing
    drift_critical_pct: f64,
    /// Severe drift — triggers emergency rebalancing
    drift_emergency_pct: f64,
    /// Maximum impermanent loss before warning
    il_warning_threshold_pct: f64,
    /// Maximum impermanent loss before circuit breaker
    il_critical_threshold_pct: f64,
    /// Protocol health score below which warning is raised
    health_warning_threshold: f64,
    /// Protocol health score below which emergency withdrawal is triggered
    health_emergency_threshold: f64,
    /// Governance approval required for rebalancing above this amount
    governance_approval_threshold: BigDecimal,
    /// Max retry attempts for failed rebalancing operations
    max_rebalancing_retries: u32,
}

impl PositionMonitoringService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self {
            db,
            drift_tolerance_pct: 2.0,
            drift_warning_pct: 4.0,
            drift_critical_pct: 5.0,
            drift_emergency_pct: 10.0,
            il_warning_threshold_pct: 5.0,
            il_critical_threshold_pct: 10.0,
            health_warning_threshold: 0.6,
            health_emergency_threshold: 0.3,
            governance_approval_threshold: BigDecimal::from(100_000),
            max_rebalancing_retries: 3,
        }
    }

    // ── Drift Calculation ─────────────────────────────────────────────────────

    /// Compute drift magnitude and severity for a single protocol allocation
    pub fn compute_drift(
        &self,
        target_pct: f64,
        current_pct: f64,
    ) -> (f64, DriftSeverity) {
        let drift = (current_pct - target_pct).abs();
        let severity = if drift >= self.drift_emergency_pct {
            DriftSeverity::SevereDrift
        } else if drift >= self.drift_critical_pct {
            DriftSeverity::ExceedingTolerance
        } else if drift >= self.drift_warning_pct {
            DriftSeverity::ApproachingTolerance
        } else {
            DriftSeverity::WithinTolerance
        };
        (drift, severity)
    }

    /// Compute drift status for all protocols in a strategy
    pub fn compute_strategy_drift(
        &self,
        strategy_id: Uuid,
        target_allocations: &HashMap<String, f64>,
        current_allocations: &HashMap<String, f64>,
    ) -> StrategyDriftStatus {
        let mut protocol_drifts = Vec::new();
        let mut requires_rebalancing = false;

        for (protocol_id, &target_pct) in target_allocations {
            let current_pct = current_allocations.get(protocol_id).copied().unwrap_or(0.0);
            let (drift_magnitude_pct, severity) = self.compute_drift(target_pct, current_pct);

            if severity == DriftSeverity::ExceedingTolerance || severity == DriftSeverity::SevereDrift {
                requires_rebalancing = true;
            }

            protocol_drifts.push(ProtocolDriftStatus {
                protocol_id: protocol_id.clone(),
                target_allocation_pct: target_pct,
                current_allocation_pct: current_pct,
                drift_magnitude_pct,
                severity,
            });
        }

        StrategyDriftStatus {
            strategy_id,
            computed_at: Utc::now(),
            protocol_drifts,
            requires_rebalancing,
        }
    }

    // ── Rebalancing Plan ──────────────────────────────────────────────────────

    /// Compute a rebalancing plan from current drift status
    pub fn compute_rebalancing_plan(
        &self,
        strategy_id: Uuid,
        trigger_reason: &str,
        target_allocations: &HashMap<String, f64>,
        current_values: &HashMap<String, BigDecimal>,
        total_value: &BigDecimal,
    ) -> RebalancingPlan {
        let mut operations = Vec::new();
        let mut total_rebalancing_amount = BigDecimal::from(0);

        for (protocol_id, &target_pct) in target_allocations {
            let current_value = current_values.get(protocol_id).cloned().unwrap_or_default();
            let target_value = total_value * BigDecimal::from((target_pct / 100.0 * 1e8) as i64)
                / BigDecimal::from(1_000_000_00_i64);

            let diff = target_value.clone() - current_value.clone();
            if diff > BigDecimal::from(0) {
                total_rebalancing_amount = total_rebalancing_amount.clone() + diff.clone();
                operations.push(RebalancingOperation {
                    protocol_id: protocol_id.clone(),
                    operation_type: RebalancingOperationType::Deposit,
                    amount: diff,
                });
            } else if diff < BigDecimal::from(0) {
                let withdraw_amount = -diff.clone();
                total_rebalancing_amount = total_rebalancing_amount.clone() + withdraw_amount.clone();
                operations.push(RebalancingOperation {
                    protocol_id: protocol_id.clone(),
                    operation_type: RebalancingOperationType::Withdraw,
                    amount: withdraw_amount,
                });
            }
        }

        // Sort: withdrawals first, then deposits
        operations.sort_by(|a, b| {
            use RebalancingOperationType::*;
            match (&a.operation_type, &b.operation_type) {
                (Withdraw, Deposit) => std::cmp::Ordering::Less,
                (Deposit, Withdraw) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });

        let requires_governance_approval = total_rebalancing_amount > self.governance_approval_threshold;

        RebalancingPlan {
            plan_id: Uuid::new_v4(),
            strategy_id,
            trigger_reason: trigger_reason.to_string(),
            operations,
            requires_governance_approval,
            created_at: Utc::now(),
        }
    }

    // ── Impermanent Loss ──────────────────────────────────────────────────────

    /// Compute impermanent loss percentage for an AMM position
    /// Uses the standard IL formula: IL = 2*sqrt(k) / (1+k) - 1
    /// where k = current_price / deposit_price
    pub fn compute_impermanent_loss(&self, deposit_price_ratio: f64, current_price_ratio: f64) -> f64 {
        if deposit_price_ratio <= 0.0 {
            return 0.0;
        }
        let k = current_price_ratio / deposit_price_ratio;
        let il = 2.0 * k.sqrt() / (1.0 + k) - 1.0;
        il.abs() * 100.0 // return as positive percentage
    }

    /// Determine alert level for impermanent loss
    pub fn il_alert_level(&self, il_pct: f64) -> MonitoringAlertLevel {
        if il_pct >= self.il_critical_threshold_pct {
            MonitoringAlertLevel::Critical
        } else if il_pct >= self.il_warning_threshold_pct {
            MonitoringAlertLevel::Warning
        } else {
            MonitoringAlertLevel::Informational
        }
    }

    // ── Protocol Health ───────────────────────────────────────────────────────

    /// Determine alert level from protocol health score
    pub fn health_alert_level(&self, health_score: f64) -> MonitoringAlertLevel {
        if health_score < self.health_emergency_threshold {
            MonitoringAlertLevel::Emergency
        } else if health_score < self.health_warning_threshold {
            MonitoringAlertLevel::Warning
        } else {
            MonitoringAlertLevel::Informational
        }
    }

    // ── Adaptive Monitoring Frequency ────────────────────────────────────────

    /// Return monitoring interval in seconds based on position value and alert level
    pub fn monitoring_interval_secs(
        &self,
        position_value: &BigDecimal,
        alert_level: &MonitoringAlertLevel,
        high_value_threshold: &BigDecimal,
    ) -> u64 {
        let base = if position_value >= high_value_threshold {
            300 // 5 minutes for high-value positions
        } else {
            1800 // 30 minutes for smaller positions
        };

        match alert_level {
            MonitoringAlertLevel::Emergency => 60,
            MonitoringAlertLevel::Critical => base / 3,
            MonitoringAlertLevel::Warning => base / 2,
            MonitoringAlertLevel::Informational => base,
        }
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

pub struct PositionMonitoringHandlers;

impl PositionMonitoringHandlers {
    /// GET /api/admin/defi/positions
    pub async fn list_positions(
        State(svc): State<Arc<PositionMonitoringService>>,
    ) -> Result<Json<Vec<PositionSummary>>, AppError> {
        let rows = sqlx::query_as::<_, PositionSummary>(
            r#"
            SELECT p.position_id, p.protocol_id, p.strategy_id,
                   p.current_value, p.position_status,
                   s.allocation_drift_pct, s.impermanent_loss_pct,
                   s.protocol_health_score, s.alert_level, s.snapshotted_at
            FROM defi_positions p
            LEFT JOIN LATERAL (
                SELECT * FROM defi_position_snapshots
                WHERE position_id = p.position_id
                ORDER BY snapshotted_at DESC LIMIT 1
            ) s ON true
            WHERE p.position_status = 'active'
            "#,
        )
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(rows))
    }

    /// GET /api/admin/defi/positions/:position_id
    pub async fn get_position(
        State(svc): State<Arc<PositionMonitoringService>>,
        Path(position_id): Path<Uuid>,
    ) -> Result<Json<PositionDetail>, AppError> {
        let snapshots = sqlx::query_as::<_, PositionSnapshot>(
            "SELECT * FROM defi_position_snapshots WHERE position_id = $1 ORDER BY snapshotted_at DESC LIMIT 100",
        )
        .bind(position_id)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        let rebalancing_history = sqlx::query_as::<_, RebalancingAuditEvent>(
            "SELECT * FROM defi_rebalancing_audit WHERE strategy_id IN (SELECT strategy_id FROM defi_positions WHERE position_id = $1) ORDER BY started_at DESC LIMIT 20",
        )
        .bind(position_id)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        Ok(Json(PositionDetail {
            position_id,
            snapshots,
            rebalancing_history,
        }))
    }

    /// GET /api/admin/defi/positions/:position_id/value-history
    pub async fn get_value_history(
        State(svc): State<Arc<PositionMonitoringService>>,
        Path(position_id): Path<Uuid>,
        Query(params): Query<ValueHistoryParams>,
    ) -> Result<Json<Vec<PositionSnapshot>>, AppError> {
        let limit = params.limit.unwrap_or(200);
        let rows = sqlx::query_as::<_, PositionSnapshot>(
            "SELECT * FROM defi_position_snapshots WHERE position_id = $1 ORDER BY snapshotted_at DESC LIMIT $2",
        )
        .bind(position_id)
        .bind(limit)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(rows))
    }

    /// GET /api/admin/defi/positions/:position_id/impermanent-loss
    pub async fn get_impermanent_loss(
        State(svc): State<Arc<PositionMonitoringService>>,
        Path(position_id): Path<Uuid>,
    ) -> Result<Json<ImpermanentLossRecord>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT position_id, pool_id, current_il_pct, cumulative_il_pct,
                   deposit_price_ratio, current_price_ratio, computed_at
            FROM defi_impermanent_loss_records
            WHERE position_id = $1
            ORDER BY computed_at DESC LIMIT 1
            "#,
            position_id
        )
        .fetch_optional(svc.db.as_ref())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("Impermanent loss record not found".into()))?;

        Ok(Json(ImpermanentLossRecord {
            position_id: row.position_id,
            pool_id: row.pool_id,
            current_il_pct: row.current_il_pct,
            cumulative_il_pct: row.cumulative_il_pct,
            deposit_price_ratio: row.deposit_price_ratio,
            current_price_ratio: row.current_price_ratio,
            computed_at: row.computed_at,
        }))
    }

    /// GET /api/admin/defi/strategies/:strategy_id/drift-status
    pub async fn get_drift_status(
        State(svc): State<Arc<PositionMonitoringService>>,
        Path(strategy_id): Path<Uuid>,
    ) -> Result<Json<StrategyDriftStatus>, AppError> {
        // Fetch target allocations
        let targets = sqlx::query!(
            "SELECT protocol_id, target_allocation_percentage FROM defi_strategy_allocations WHERE strategy_id = $1",
            strategy_id
        )
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        let target_map: HashMap<String, f64> = targets
            .into_iter()
            .map(|r| (r.protocol_id, r.target_allocation_percentage))
            .collect();

        // Fetch current allocations
        let current = sqlx::query!(
            r#"
            SELECT protocol_id,
                   SUM(current_value) / NULLIF(SUM(SUM(current_value)) OVER (), 0) * 100 AS current_pct
            FROM defi_positions
            WHERE strategy_id = $1 AND position_status = 'active'
            GROUP BY protocol_id
            "#,
            strategy_id
        )
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;

        let current_map: HashMap<String, f64> = current
            .into_iter()
            .map(|r| (r.protocol_id, r.current_pct.unwrap_or(0.0)))
            .collect();

        let drift_status = svc.compute_strategy_drift(strategy_id, &target_map, &current_map);
        Ok(Json(drift_status))
    }

    /// GET /api/admin/defi/protocols/:protocol_id/health-history
    pub async fn get_protocol_health_history(
        State(svc): State<Arc<PositionMonitoringService>>,
        Path(protocol_id): Path<String>,
        Query(params): Query<ValueHistoryParams>,
    ) -> Result<Json<Vec<ProtocolHealthHistory>>, AppError> {
        let limit = params.limit.unwrap_or(200);
        let rows = sqlx::query_as::<_, ProtocolHealthHistory>(
            "SELECT * FROM defi_protocol_health_history WHERE protocol_id = $1 ORDER BY recorded_at DESC LIMIT $2",
        )
        .bind(&protocol_id)
        .bind(limit)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(rows))
    }

    /// GET /api/admin/defi/monitoring/alerts
    pub async fn list_alerts(
        State(svc): State<Arc<PositionMonitoringService>>,
    ) -> Result<Json<Vec<MonitoringAlert>>, AppError> {
        let alerts = sqlx::query_as::<_, MonitoringAlert>(
            "SELECT * FROM defi_monitoring_alerts WHERE resolved_at IS NULL ORDER BY created_at DESC",
        )
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(alerts))
    }

    /// POST /api/admin/defi/monitoring/alerts/:alert_id/acknowledge
    pub async fn acknowledge_alert(
        State(svc): State<Arc<PositionMonitoringService>>,
        Path(alert_id): Path<Uuid>,
        Json(req): Json<AcknowledgeAlertRequest>,
    ) -> Result<StatusCode, AppError> {
        sqlx::query!(
            "UPDATE defi_monitoring_alerts SET acknowledged_by = $1, acknowledged_at = NOW() WHERE alert_id = $2",
            req.assigned_to,
            alert_id
        )
        .execute(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(StatusCode::OK)
    }

    /// POST /api/admin/defi/monitoring/alerts/:alert_id/resolve
    pub async fn resolve_alert(
        State(svc): State<Arc<PositionMonitoringService>>,
        Path(alert_id): Path<Uuid>,
        Json(req): Json<ResolveAlertRequest>,
    ) -> Result<StatusCode, AppError> {
        sqlx::query!(
            "UPDATE defi_monitoring_alerts SET resolved_by = $1, resolved_at = NOW(), resolution_notes = $2 WHERE alert_id = $3",
            req.resolved_by,
            req.resolution_notes,
            alert_id
        )
        .execute(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(StatusCode::OK)
    }

    /// GET /api/admin/defi/rebalancing/history
    pub async fn list_rebalancing_history(
        State(svc): State<Arc<PositionMonitoringService>>,
        Query(params): Query<RebalancingHistoryParams>,
    ) -> Result<Json<Vec<RebalancingAuditEvent>>, AppError> {
        let limit = params.limit.unwrap_or(50);
        let rows = sqlx::query_as::<_, RebalancingAuditEvent>(
            "SELECT * FROM defi_rebalancing_audit ORDER BY started_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(svc.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(rows))
    }

    /// GET /api/admin/defi/rebalancing/history/:event_id
    pub async fn get_rebalancing_event(
        State(svc): State<Arc<PositionMonitoringService>>,
        Path(event_id): Path<Uuid>,
    ) -> Result<Json<RebalancingAuditEvent>, AppError> {
        let event = sqlx::query_as::<_, RebalancingAuditEvent>(
            "SELECT * FROM defi_rebalancing_audit WHERE event_id = $1",
        )
        .bind(event_id)
        .fetch_optional(svc.db.as_ref())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("Rebalancing event not found".into()))?;
        Ok(Json(event))
    }
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PositionSummary {
    pub position_id: Uuid,
    pub protocol_id: String,
    pub strategy_id: Option<Uuid>,
    pub current_value: BigDecimal,
    pub position_status: String,
    pub allocation_drift_pct: Option<f64>,
    pub impermanent_loss_pct: Option<f64>,
    pub protocol_health_score: Option<f64>,
    pub alert_level: Option<MonitoringAlertLevel>,
    pub snapshotted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct PositionDetail {
    pub position_id: Uuid,
    pub snapshots: Vec<PositionSnapshot>,
    pub rebalancing_history: Vec<RebalancingAuditEvent>,
}

#[derive(Debug, Deserialize)]
pub struct ValueHistoryParams {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RebalancingHistoryParams {
    pub strategy_id: Option<Uuid>,
    pub trigger_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AcknowledgeAlertRequest {
    pub assigned_to: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveAlertRequest {
    pub resolved_by: String,
    pub resolution_notes: String,
}

// ── Routes ────────────────────────────────────────────────────────────────────

pub fn position_monitoring_routes(svc: Arc<PositionMonitoringService>) -> Router {
    Router::new()
        .route("/positions", get(PositionMonitoringHandlers::list_positions))
        .route("/positions/:position_id", get(PositionMonitoringHandlers::get_position))
        .route("/positions/:position_id/value-history", get(PositionMonitoringHandlers::get_value_history))
        .route("/positions/:position_id/impermanent-loss", get(PositionMonitoringHandlers::get_impermanent_loss))
        .route("/strategies/:strategy_id/drift-status", get(PositionMonitoringHandlers::get_drift_status))
        .route("/protocols/:protocol_id/health-history", get(PositionMonitoringHandlers::get_protocol_health_history))
        .route("/monitoring/alerts", get(PositionMonitoringHandlers::list_alerts))
        .route("/monitoring/alerts/:alert_id/acknowledge", post(PositionMonitoringHandlers::acknowledge_alert))
        .route("/monitoring/alerts/:alert_id/resolve", post(PositionMonitoringHandlers::resolve_alert))
        .route("/rebalancing/history", get(PositionMonitoringHandlers::list_rebalancing_history))
        .route("/rebalancing/history/:event_id", get(PositionMonitoringHandlers::get_rebalancing_event))
        .with_state(svc)
}

// ── Observability ─────────────────────────────────────────────────────────────

pub fn record_monitoring_cycle(strategy_id: Uuid, positions_checked: usize, alerts_raised: usize) {
    tracing::info!(
        strategy_id = %strategy_id,
        positions_checked = positions_checked,
        alerts_raised = alerts_raised,
        "DeFi position monitoring cycle completed"
    );
}

pub fn record_drift_detected(strategy_id: Uuid, protocol_id: &str, drift_pct: f64, severity: &DriftSeverity) {
    tracing::warn!(
        strategy_id = %strategy_id,
        protocol_id = %protocol_id,
        drift_pct = drift_pct,
        severity = ?severity,
        "DeFi allocation drift detected"
    );
}

pub fn record_rebalancing_triggered(strategy_id: Uuid, trigger: &RebalancingTriggerType) {
    tracing::warn!(
        strategy_id = %strategy_id,
        trigger = ?trigger,
        "DeFi rebalancing triggered"
    );
}

pub fn record_emergency_rebalancing(strategy_id: Uuid, reason: &str) {
    tracing::error!(
        strategy_id = %strategy_id,
        reason = %reason,
        "DeFi emergency rebalancing triggered"
    );
}

pub fn record_rebalancing_failure(strategy_id: Uuid, protocol_id: &str, attempt: u32, error: &str) {
    tracing::error!(
        strategy_id = %strategy_id,
        protocol_id = %protocol_id,
        attempt = attempt,
        error = %error,
        "DeFi rebalancing operation failed"
    );
}

// ── Unit Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_svc() -> PositionMonitoringService {
        PositionMonitoringService {
            db: unsafe { Arc::from_raw(std::ptr::NonNull::dangling().as_ptr()) },
            drift_tolerance_pct: 2.0,
            drift_warning_pct: 4.0,
            drift_critical_pct: 5.0,
            drift_emergency_pct: 10.0,
            il_warning_threshold_pct: 5.0,
            il_critical_threshold_pct: 10.0,
            health_warning_threshold: 0.6,
            health_emergency_threshold: 0.3,
            governance_approval_threshold: BigDecimal::from(100_000),
            max_rebalancing_retries: 3,
        }
    }

    #[test]
    fn test_drift_severity_classification() {
        let svc = make_svc();
        assert_eq!(svc.compute_drift(50.0, 51.5).1, DriftSeverity::WithinTolerance);
        assert_eq!(svc.compute_drift(50.0, 54.0).1, DriftSeverity::ApproachingTolerance);
        assert_eq!(svc.compute_drift(50.0, 55.5).1, DriftSeverity::ExceedingTolerance);
        assert_eq!(svc.compute_drift(50.0, 61.0).1, DriftSeverity::SevereDrift);
    }

    #[test]
    fn test_drift_magnitude_calculation() {
        let svc = make_svc();
        let (magnitude, _) = svc.compute_drift(40.0, 47.0);
        assert!((magnitude - 7.0).abs() < 0.001);
    }

    #[test]
    fn test_impermanent_loss_formula() {
        let svc = make_svc();
        // No price change → zero IL
        let il = svc.compute_impermanent_loss(1.0, 1.0);
        assert!(il.abs() < 0.001);

        // Price doubles → ~5.72% IL
        let il = svc.compute_impermanent_loss(1.0, 2.0);
        assert!((il - 5.72).abs() < 0.1);

        // Price halves → same IL by symmetry
        let il2 = svc.compute_impermanent_loss(1.0, 0.5);
        assert!((il - il2).abs() < 0.01);
    }

    #[test]
    fn test_il_alert_level() {
        let svc = make_svc();
        assert_eq!(svc.il_alert_level(2.0), MonitoringAlertLevel::Informational);
        assert_eq!(svc.il_alert_level(6.0), MonitoringAlertLevel::Warning);
        assert_eq!(svc.il_alert_level(12.0), MonitoringAlertLevel::Critical);
    }

    #[test]
    fn test_health_alert_level() {
        let svc = make_svc();
        assert_eq!(svc.health_alert_level(0.9), MonitoringAlertLevel::Informational);
        assert_eq!(svc.health_alert_level(0.5), MonitoringAlertLevel::Warning);
        assert_eq!(svc.health_alert_level(0.2), MonitoringAlertLevel::Emergency);
    }

    #[test]
    fn test_adaptive_monitoring_frequency() {
        let svc = make_svc();
        let high_threshold = BigDecimal::from(50_000);
        let high_value = BigDecimal::from(100_000);
        let low_value = BigDecimal::from(1_000);

        // High value, informational → 5 min
        assert_eq!(
            svc.monitoring_interval_secs(&high_value, &MonitoringAlertLevel::Informational, &high_threshold),
            300
        );
        // Low value, informational → 30 min
        assert_eq!(
            svc.monitoring_interval_secs(&low_value, &MonitoringAlertLevel::Informational, &high_threshold),
            1800
        );
        // Emergency → always 60s
        assert_eq!(
            svc.monitoring_interval_secs(&low_value, &MonitoringAlertLevel::Emergency, &high_threshold),
            60
        );
    }

    #[test]
    fn test_rebalancing_plan_withdrawals_first() {
        let svc = make_svc();
        let strategy_id = Uuid::new_v4();
        let mut targets = HashMap::new();
        targets.insert("proto_a".to_string(), 60.0);
        targets.insert("proto_b".to_string(), 40.0);

        let mut current_values = HashMap::new();
        current_values.insert("proto_a".to_string(), BigDecimal::from(700));
        current_values.insert("proto_b".to_string(), BigDecimal::from(300));

        let plan = svc.compute_rebalancing_plan(
            strategy_id,
            "drift_based",
            &targets,
            &current_values,
            &BigDecimal::from(1000),
        );

        // Withdrawals should come before deposits
        let first_withdraw_idx = plan.operations.iter().position(|o| o.operation_type == RebalancingOperationType::Withdraw);
        let first_deposit_idx = plan.operations.iter().position(|o| o.operation_type == RebalancingOperationType::Deposit);
        if let (Some(w), Some(d)) = (first_withdraw_idx, first_deposit_idx) {
            assert!(w < d, "Withdrawals must precede deposits");
        }
    }

    #[test]
    fn test_strategy_drift_requires_rebalancing() {
        let svc = make_svc();
        let strategy_id = Uuid::new_v4();
        let mut targets = HashMap::new();
        targets.insert("proto_a".to_string(), 50.0);

        let mut current = HashMap::new();
        current.insert("proto_a".to_string(), 58.0); // 8% drift → exceeds critical

        let status = svc.compute_strategy_drift(strategy_id, &targets, &current);
        assert!(status.requires_rebalancing);
    }
}
