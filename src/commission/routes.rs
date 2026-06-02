//! Commission engine route registrations (Issue #471).

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use super::handlers::{configure_commission, manual_adjust, revenue_statement, CommissionState};

pub fn commission_routes(state: Arc<CommissionState>) -> Router {
    Router::new()
        .route(
            "/api/v1/admin/partners/commissions/configure",
            post(configure_commission),
        )
        .route(
            "/api/v1/admin/partners/revenue/adjust",
            post(manual_adjust),
        )
        .route(
            "/api/v1/partners/:partner_id/revenue/statement",
            get(revenue_statement),
        )
        .with_state(state)
}
