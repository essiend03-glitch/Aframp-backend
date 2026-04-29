//! Route definitions for DR/BCP (Issue #DR-BCP).

use super::handlers::*;
use axum::{
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;

use crate::dr_bcp::service::DrBcpService;

pub fn dr_bcp_routes() -> Router<Arc<DrBcpService>> {
    Router::new()
        .route("/dr/status", get(get_dr_status))
        .route("/dr/incidents", post(declare_incident))
        .route("/dr/incidents/:id/status", patch(update_incident_status))
        .route("/dr/incidents/:id/notify-regulator", post(notify_regulator))
        .route("/dr/restore-tests", post(record_restore_test))
}
