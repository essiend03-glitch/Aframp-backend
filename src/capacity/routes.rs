use super::handlers::*;
use axum::{
    routing::{get, post},
    Router,
};

/// Internal routes — mount behind infra-team auth middleware.
pub fn internal_router(state: CapacityState) -> Router {
    Router::new()
        .route("/metrics",                       post(ingest_metrics))
        .route("/forecast",                      get(get_forecast))
        .route("/forecast/run",                  post(trigger_forecast))
        .route("/scenarios",                     post(run_scenario).get(list_scenarios))
        .route("/costs",                         get(get_cost_projections))
        .route("/alerts",                        get(list_alerts))
        .route("/alerts/:id/acknowledge",        post(acknowledge_alert))
        .route("/rcu/update",                    post(trigger_rcu_update))
        .route("/report/quarterly",              get(get_quarterly_report))
        .route("/report/quarterly/generate",     post(generate_quarterly_report))
        .with_state(state)
}

/// Management route — plain-language dashboard, no raw technical metrics.
pub fn management_router(state: CapacityState) -> Router {
    Router::new()
        .route("/dashboard", get(management_dashboard))
        .with_state(state)
}
