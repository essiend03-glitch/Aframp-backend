use crate::sla::{
    models::*,
    repository::SlaRepository,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// ── Shared state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SlaState {
    pub repo: Arc<SlaRepository>,
    pub pool: PgPool,
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

pub async fn get_dashboard(
    State(s): State<Arc<SlaState>>,
) -> Result<Json<SlaComplianceDashboard>, (StatusCode, String)> {
    let (open_incidents, slo_definitions, stats) = tokio::try_join!(
        s.repo.list_open_incidents(),
        s.repo.list_slos(),
        s.repo.dashboard_stats(),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (recent_breaches_30d, mttr_seconds_30d, availability_pct_30d) = stats;

    Ok(Json(SlaComplianceDashboard {
        open_incidents,
        slo_definitions,
        recent_breaches_30d,
        mttr_seconds_30d,
        availability_pct_30d,
    }))
}

// ── Incidents ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct IncidentQuery {
    pub status: Option<String>,
}

pub async fn list_incidents(
    State(s): State<Arc<SlaState>>,
    Query(q): Query<IncidentQuery>,
) -> Result<Json<Vec<SlaBreachIncident>>, (StatusCode, String)> {
    match q.status.as_deref() {
        None | Some("open") => s.repo.list_open_incidents().await,
        Some(status) => s.repo.list_incidents_by_status(Some(status)).await,
    }
    .map(Json)
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn get_incident(
    State(s): State<Arc<SlaState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<SlaBreachIncident>, (StatusCode, String)> {
    s.repo
        .get_incident(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Incident not found".into()))
}

pub async fn update_incident(
    State(s): State<Arc<SlaState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateIncidentRequest>,
) -> Result<Json<SlaBreachIncident>, (StatusCode, String)> {
    s.repo
        .update_incident(id, &req)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

// ── Post-mortems ──────────────────────────────────────────────────────────────

pub async fn create_post_mortem(
    State(s): State<Arc<SlaState>>,
    Path(incident_id): Path<Uuid>,
    Json(req): Json<CreatePostMortemRequest>,
) -> Result<Json<SlaPostMortem>, (StatusCode, String)> {
    // Verify incident exists
    s.repo
        .get_incident(incident_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Incident not found".into()))?;

    // Transition incident to post_mortem_pending
    let _ = s
        .repo
        .update_incident(
            incident_id,
            &UpdateIncidentRequest {
                status: Some("post_mortem_pending".into()),
                root_cause_summary: None,
                remediation_steps: None,
                etr: None,
            },
        )
        .await;

    s.repo
        .create_post_mortem(incident_id, &req)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn get_post_mortem(
    State(s): State<Arc<SlaState>>,
    Path(incident_id): Path<Uuid>,
) -> Result<Json<SlaPostMortem>, (StatusCode, String)> {
    s.repo
        .get_post_mortem(incident_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Post-mortem not found".into()))
}

// ── Compliance reports ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ReportQuery {
    pub partner_id: Option<Uuid>,
    pub month: Option<NaiveDate>, // YYYY-MM-DD (first of month)
}

pub async fn list_reports(
    State(s): State<Arc<SlaState>>,
    Query(q): Query<ReportQuery>,
) -> Result<Json<Vec<SlaComplianceReport>>, (StatusCode, String)> {
    s.repo
        .list_reports(q.partner_id)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn generate_report(
    State(s): State<Arc<SlaState>>,
    Query(q): Query<ReportQuery>,
) -> Result<Json<SlaComplianceReport>, (StatusCode, String)> {
    let month = q.month.unwrap_or_else(|| {
        let now = chrono::Utc::now();
        NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap()
    });
    s.repo
        .generate_monthly_report(q.partner_id, month)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

// ── SLO definitions ───────────────────────────────────────────────────────────

pub async fn list_slos(
    State(s): State<Arc<SlaState>>,
) -> Result<Json<Vec<SloDefinition>>, (StatusCode, String)> {
    s.repo
        .list_slos()
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn sla_routes(state: Arc<SlaState>) -> axum::Router {
    use axum::routing::{get, patch, post};
    axum::Router::new()
        // Dashboard
        .route("/api/admin/sla/dashboard",          get(get_dashboard))
        // SLO definitions
        .route("/api/admin/sla/slos",               get(list_slos))
        // Incidents
        .route("/api/admin/sla/incidents",          get(list_incidents))
        .route("/api/admin/sla/incidents/:id",      get(get_incident).patch(update_incident))
        // Post-mortems
        .route("/api/admin/sla/incidents/:id/post-mortem",
               post(create_post_mortem).get(get_post_mortem))
        // Compliance reports
        .route("/api/admin/sla/reports",            get(list_reports))
        .route("/api/admin/sla/reports/generate",   post(generate_report))
        .with_state(state)
}
