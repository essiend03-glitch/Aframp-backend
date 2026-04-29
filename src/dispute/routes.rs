//! Route definitions for Merchant Dispute Resolution (Issue #337).

use super::handlers::*;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use crate::dispute::service::DisputeService;

pub fn dispute_routes() -> Router<Arc<DisputeService>> {
    Router::new()
        // Customer — open & track disputes
        .route("/disputes", post(open_dispute))
        .route("/disputes/customer/:wallet", get(list_customer_disputes))
        .route("/disputes/:id/evidence/customer", post(submit_customer_evidence))
        // Merchant — mediation dashboard
        .route("/disputes/merchant/:merchant_id", get(list_merchant_disputes))
        .route("/disputes/:id/respond", post(merchant_respond))
        .route("/disputes/:id/evidence/merchant", post(submit_merchant_evidence))
        // Platform mediation
        .route("/disputes/:id/resolve", post(resolve_dispute))
        // Shared read
        .route("/disputes/:id", get(get_dispute))
        .route("/disputes/:id/evidence", get(list_evidence))
        .route("/disputes/:id/audit", get(get_audit_log))
}
