//! HTTP handlers for the Risk Management module — Issue #494.

use crate::risk::{
    models::*,
    repository::RiskRepository,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct RiskState {
    pub repo: Arc<RiskRepository>,
    pub pool: PgPool,
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

pub async fn get_dashboard(
    State(s): State<Arc<RiskState>>,
) -> Result<Json<RiskDashboard>, (StatusCode, String)> {
    let (profiles, active_circuit_breakers, recent_heartbeats) = tokio::try_join!(
        s.repo.list_profiles(),
        s.repo.list_active_circuit_breakers(),
        s.repo.recent_heartbeats(50),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RiskDashboard {
        profiles,
        active_circuit_breakers,
        recent_heartbeats,
    }))
}

// ── Circuit breaker release (multi-sig) ──────────────────────────────────────

pub async fn submit_release_approval(
    State(s): State<Arc<RiskState>>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<ReleaseApprovalRequest>,
) -> Result<Json<CircuitBreakerEvent>, (StatusCode, String)> {
    s.repo
        .add_release_approval(event_id, &req.officer_id, &req.signature)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn risk_routes(state: Arc<RiskState>) -> axum::Router {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/api/v1/admin/risk/dashboard", get(get_dashboard))
        .route(
            "/api/v1/admin/risk/circuit-breakers/:id/release",
            post(submit_release_approval),
        )
        .with_state(state)
}
