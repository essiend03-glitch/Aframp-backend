use crate::agent_cfo::handlers::{
    get_policy, project_cost, query_ledger, record_inference, unfreeze_agent, CfoState,
};
use axum::{routing::{get, post}, Router};

pub fn agent_cfo_routes(state: CfoState) -> Router {
    Router::new()
        .route("/agent-cfo/project-cost", post(project_cost))
        .route("/agent-cfo/inference", post(record_inference))
        .route("/agent-cfo/ledger", get(query_ledger))
        .route("/agent-cfo/policy/:agent_id", get(get_policy))
        .route("/agent-cfo/unfreeze/:agent_id", post(unfreeze_agent))
        .with_state(state)
}
