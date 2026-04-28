//! SAR route definitions

use axum::{
    routing::{get, post},
    Router,
};

use super::handlers::{approve_sar, get_audit, get_sar, list_queue, reject_sar, SarState};

pub fn router(state: SarState) -> Router {
    Router::new()
        .route("/queue", get(list_queue))
        .route("/:id", get(get_sar))
        .route("/:id/approve", post(approve_sar))
        .route("/:id/reject", post(reject_sar))
        .route("/:id/audit", get(get_audit))
        .with_state(state)
}
