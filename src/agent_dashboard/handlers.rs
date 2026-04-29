use crate::agent_dashboard::{
    service::AgentDashboardService,
    types::{
        AgentListQuery, ApprovalDecisionRequest, CreateTemplateRequest, DeployTemplateRequest,
        InterventionRequest,
    },
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

pub type DashboardState = Arc<AgentDashboardService>;

// ── Telemetry ─────────────────────────────────────────────────────────────────

/// GET /agent-dashboard/agents
pub async fn list_agents(
    State(svc): State<DashboardState>,
    Query(q): Query<AgentListQuery>,
) -> impl IntoResponse {
    match svc.repo().list_agents(&q).await {
        Ok(agents) => (StatusCode::OK, Json(serde_json::json!(agents))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-dashboard/agents/:agent_id
pub async fn get_agent(
    State(svc): State<DashboardState>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_agent(agent_id).await {
        Ok(a) => (StatusCode::OK, Json(serde_json::json!(a))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-dashboard/agents/:agent_id/tasks
pub async fn list_tasks(
    State(svc): State<DashboardState>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.repo().list_tasks(agent_id).await {
        Ok(tasks) => (StatusCode::OK, Json(serde_json::json!(tasks))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-dashboard/tasks/:task_id/trace
///
/// Reasoning Trace view — renders the agent's internal thought process.
pub async fn get_task_trace(
    State(svc): State<DashboardState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_task_trace(task_id).await {
        Ok(task) => (StatusCode::OK, Json(serde_json::json!({
            "task_id": task.id,
            "agent_id": task.agent_id,
            "description": task.description,
            "status": task.status,
            "projected_cost_cngn": task.projected_cost_cngn,
            "actual_cost_cngn": task.actual_cost_cngn,
            "reasoning_trace": task.reasoning_trace,
            "started_at": task.started_at,
            "completed_at": task.completed_at,
        }))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Intervention protocols ────────────────────────────────────────────────────

/// POST /agent-dashboard/agents/:agent_id/intervene
///
/// Accepts action: "pause" | "resume" | "reset" | "circuit_breaker"
pub async fn intervene(
    State(svc): State<DashboardState>,
    Path(agent_id): Path<Uuid>,
    Json(req): Json<InterventionRequest>,
) -> impl IntoResponse {
    let result = match req.action.as_str() {
        "pause" => {
            svc.pause_agent(agent_id, &req.performed_by, req.reason.as_deref())
                .await
        }
        "resume" => {
            svc.resume_agent(agent_id, &req.performed_by, req.reason.as_deref())
                .await
        }
        "reset" => {
            svc.reset_agent(agent_id, &req.performed_by, req.reason.as_deref())
                .await
        }
        "circuit_breaker" => {
            svc.circuit_breaker(agent_id, &req.performed_by, req.reason.as_deref())
                .await
        }
        other => Err(format!("unknown action: {other}")),
    };

    match result {
        Ok(log) => (StatusCode::OK, Json(serde_json::json!(log))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-dashboard/agents/:agent_id/interventions
pub async fn list_interventions(
    State(svc): State<DashboardState>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.repo().list_interventions(agent_id).await {
        Ok(logs) => (StatusCode::OK, Json(serde_json::json!(logs))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Human Approval Queue ──────────────────────────────────────────────────────

/// GET /agent-dashboard/approvals
pub async fn list_approvals(State(svc): State<DashboardState>) -> impl IntoResponse {
    match svc.list_pending_approvals().await {
        Ok(items) => (StatusCode::OK, Json(serde_json::json!(items))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// POST /agent-dashboard/approvals/:item_id/decide
///
/// 1-click approve or reject a high-risk task.
pub async fn decide_approval(
    State(svc): State<DashboardState>,
    Path(item_id): Path<Uuid>,
    Json(req): Json<ApprovalDecisionRequest>,
) -> impl IntoResponse {
    match svc.decide_approval(item_id, &req.decision, &req.decided_by).await {
        Ok(item) => (StatusCode::OK, Json(serde_json::json!(item))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Swarm / Template management ───────────────────────────────────────────────

/// GET /agent-dashboard/templates
pub async fn list_templates(State(svc): State<DashboardState>) -> impl IntoResponse {
    match svc.list_templates().await {
        Ok(t) => (StatusCode::OK, Json(serde_json::json!(t))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// POST /agent-dashboard/templates
pub async fn create_template(
    State(svc): State<DashboardState>,
    Json(req): Json<CreateTemplateRequest>,
) -> impl IntoResponse {
    match svc.create_template(&req.name, &req.instructions, &req.created_by).await {
        Ok(t) => (StatusCode::CREATED, Json(serde_json::json!(t))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// POST /agent-dashboard/templates/deploy
///
/// Push a template update to a fleet of agents at once.
pub async fn deploy_template(
    State(svc): State<DashboardState>,
    Json(req): Json<DeployTemplateRequest>,
) -> impl IntoResponse {
    match svc.deploy_template(req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Audit export ──────────────────────────────────────────────────────────────

/// GET /agent-dashboard/agents/:agent_id/audit-export
///
/// Returns all human interventions and agent decision logs — audit-ready JSON.
pub async fn audit_export(
    State(svc): State<DashboardState>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.audit_export(agent_id).await {
        Ok(rows) => (StatusCode::OK, Json(serde_json::json!(rows))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn err(msg: String) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({ "error": msg })),
    )
}
