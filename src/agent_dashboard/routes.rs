use crate::agent_dashboard::handlers::{
    audit_export, create_template, decide_approval, deploy_template, get_agent, get_task_trace,
    intervene, list_agents, list_approvals, list_interventions, list_tasks, list_templates,
    DashboardState,
};
use axum::{routing::{get, post}, Router};

pub fn agent_dashboard_routes(state: DashboardState) -> Router {
    Router::new()
        // Telemetry
        .route("/agent-dashboard/agents", get(list_agents))
        .route("/agent-dashboard/agents/:agent_id", get(get_agent))
        .route("/agent-dashboard/agents/:agent_id/tasks", get(list_tasks))
        .route("/agent-dashboard/tasks/:task_id/trace", get(get_task_trace))
        // Intervention protocols (HITL)
        .route("/agent-dashboard/agents/:agent_id/intervene", post(intervene))
        .route("/agent-dashboard/agents/:agent_id/interventions", get(list_interventions))
        // Human Approval Queue
        .route("/agent-dashboard/approvals", get(list_approvals))
        .route("/agent-dashboard/approvals/:item_id/decide", post(decide_approval))
        // Swarm / Template management
        .route("/agent-dashboard/templates", get(list_templates).post(create_template))
        .route("/agent-dashboard/templates/deploy", post(deploy_template))
        // Audit export
        .route("/agent-dashboard/agents/:agent_id/audit-export", get(audit_export))
        .with_state(state)
}
