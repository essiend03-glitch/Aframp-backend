/// HTTP Handlers — Capacity Planning & Forecasting Engine
///
///   Internal (infra-team auth):
///     POST  /capacity/metrics                  — ingest daily business metrics
///     GET   /capacity/monitor                  — internal resource monitor
///     GET   /capacity/forecast                 — time-series forecast output
///     POST  /capacity/scenarios                — run what-if simulation
///     GET   /capacity/scenarios                — list past scenarios
///     GET   /capacity/costs                    — cloud cost projections
///     GET   /capacity/alerts                   — capacity alerts
///     POST  /capacity/alerts/:id/acknowledge   — acknowledge an alert
///     POST  /capacity/rcu/update               — trigger RCU model update
///     POST  /capacity/forecast/run             — trigger forecast run
///     GET   /capacity/report/quarterly         — latest quarterly report
///     POST  /capacity/report/quarterly/generate — generate quarterly report
///
///   Management (read-only, plain-language):
///     GET   /capacity/dashboard                — management dashboard
use super::engine::CapacityEngine;
use super::types::*;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

pub type CapacityState = Arc<CapacityEngine>;

// ── Metrics ───────────────────────────────────────────────────────────────────

pub async fn ingest_metrics(
    State(engine): State<CapacityState>,
    Json(req): Json<IngestMetricsRequest>,
) -> impl IntoResponse {
    match engine.ingest_metrics(req).await {
        Ok(row) => (StatusCode::CREATED, Json(serde_json::json!({ "data": row }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// ── Forecast ──────────────────────────────────────────────────────────────────

pub async fn get_forecast(
    State(engine): State<CapacityState>,
    Query(q): Query<ForecastQuery>,
) -> impl IntoResponse {
    let horizon = match q.horizon.as_deref().unwrap_or("90d") {
        "12m" => ForecastHorizon::Annual12m,
        _ => ForecastHorizon::Rolling90d,
    };
    match engine.get_forecasts(horizon).await {
        Ok(forecasts) => (StatusCode::OK, Json(serde_json::json!({ "data": forecasts }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

pub async fn trigger_forecast(State(engine): State<CapacityState>) -> impl IntoResponse {
    match engine.run_forecasts().await {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({ "forecasts_written": n }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// ── Scenarios ─────────────────────────────────────────────────────────────────

pub async fn run_scenario(
    State(engine): State<CapacityState>,
    Extension(caller_id): Extension<String>,
    Json(req): Json<RunScenarioRequest>,
) -> impl IntoResponse {
    match engine.run_scenario(req, &caller_id).await {
        Ok(s) => (StatusCode::CREATED, Json(s)).into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

pub async fn list_scenarios(State(engine): State<CapacityState>) -> impl IntoResponse {
    match engine.repo().list_scenarios(20).await {
        Ok(s) => (StatusCode::OK, Json(serde_json::json!({ "data": s }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ── Costs ─────────────────────────────────────────────────────────────────────

pub async fn get_cost_projections(State(engine): State<CapacityState>) -> impl IntoResponse {
    match engine.project_costs(12, "aws").await {
        Ok(costs) => (StatusCode::OK, Json(serde_json::json!({ "data": costs }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// ── Alerts ────────────────────────────────────────────────────────────────────

pub async fn list_alerts(
    State(engine): State<CapacityState>,
    Query(q): Query<AlertQuery>,
) -> impl IntoResponse {
    match engine.repo().list_alerts(q.resolved).await {
        Ok(alerts) => (StatusCode::OK, Json(serde_json::json!({ "data": alerts }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

pub async fn acknowledge_alert(
    State(engine): State<CapacityState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AcknowledgeAlertRequest>,
) -> impl IntoResponse {
    match engine.repo().acknowledge_alert(id, &req.acknowledged_by, req.review_task_id.as_deref()).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "acknowledged": true }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ── RCU model ─────────────────────────────────────────────────────────────────

pub async fn trigger_rcu_update(State(engine): State<CapacityState>) -> impl IntoResponse {
    match engine.update_rcu_model().await {
        Ok(rcu) => (StatusCode::OK, Json(rcu)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// ── Quarterly report ──────────────────────────────────────────────────────────

pub async fn get_quarterly_report(State(engine): State<CapacityState>) -> impl IntoResponse {
    match engine.repo().latest_quarterly_report().await {
        Ok(Some(r)) => (StatusCode::OK, Json(r)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "No report yet" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

pub async fn generate_quarterly_report(State(engine): State<CapacityState>) -> impl IntoResponse {
    match engine.generate_quarterly_report().await {
        Ok(r) => (StatusCode::CREATED, Json(r)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// ── Management dashboard ──────────────────────────────────────────────────────

pub async fn management_dashboard(State(engine): State<CapacityState>) -> impl IntoResponse {
    match engine.management_dashboard().await {
        Ok(d) => (StatusCode::OK, Json(d)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}
