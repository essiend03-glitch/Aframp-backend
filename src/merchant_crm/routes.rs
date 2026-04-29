//! Route definitions for Merchant CRM (Issue #334).

use super::handlers::*;
use axum::{
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;
use crate::merchant_crm::service::MerchantCrmService;

pub fn merchant_crm_routes() -> Router<Arc<MerchantCrmService>> {
    Router::new()
        // Customer profiles
        .route("/merchant/:merchant_id/crm/customers", post(opt_in_customer))
        .route("/merchant/:merchant_id/crm/customers", get(list_customers))
        .route("/merchant/:merchant_id/crm/customers/:wallet_address", get(get_customer))
        .route("/merchant/:merchant_id/crm/customers/:wallet_address/tags", patch(update_tags))
        // Retention
        .route("/merchant/:merchant_id/crm/retention", get(get_retention))
        // Segments
        .route("/merchant/:merchant_id/crm/segments", post(create_segment))
        .route("/merchant/:merchant_id/crm/segments", get(list_segments))
        // Export
        .route("/merchant/:merchant_id/crm/export/anonymised", get(export_anonymised))
}
