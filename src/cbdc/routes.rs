use crate::cbdc::handlers::*;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

pub type CbdcApiState = Arc<CbdcHandlerState>;

/// Public CBDC API routes — swap initiation and status queries.
pub fn cbdc_api_routes(state: CbdcApiState) -> Router {
    Router::new()
        .route("/api/v1/cbdc/swaps", post(initiate_swap).get(list_swaps))
        .route("/api/v1/cbdc/swaps/{id}", get(get_swap_status))
        .route("/api/v1/cbdc/swaps/{id}/signatories", get(get_swap_signatories))
        .with_state(state)
}

/// Admin CBDC routes — gateway management.
pub fn cbdc_admin_routes(state: CbdcApiState) -> Router {
    Router::new()
        .route("/api/admin/cbdc/gateways", get(list_gateways).post(register_gateway))
        .route("/api/admin/cbdc/gateways/{id}", get(get_gateway))
        .with_state(state)
}
