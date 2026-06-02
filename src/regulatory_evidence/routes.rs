use crate::regulatory_evidence::handlers::*;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn regulatory_evidence_routes(state: Arc<RegulatoryEvidenceState>) -> Router {
    Router::new()
        // Evidence packages
        .route("/api/v1/regulatory-evidence/packages", post(generate_package))
        .route("/api/v1/regulatory-evidence/packages", get(list_packages))
        .route("/api/v1/regulatory-evidence/packages/:id", get(get_package))
        // Policy history
        .route("/api/v1/regulatory-evidence/policies", post(record_policy_snapshot))
        .route("/api/v1/regulatory-evidence/policies", get(list_policy_names))
        .route("/api/v1/regulatory-evidence/policies/point-in-time", get(policy_at_point_in_time))
        .route("/api/v1/regulatory-evidence/policies/:name/history", get(list_policy_history))
        // System test reports
        .route("/api/v1/regulatory-evidence/test-reports", post(record_test_report))
        .route("/api/v1/regulatory-evidence/test-reports", get(list_test_reports))
        .with_state(state)
}
