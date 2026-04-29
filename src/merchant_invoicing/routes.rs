//! Route definitions for Merchant Invoicing (Issue #333).

use super::handlers::*;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use crate::merchant_invoicing::service::MerchantInvoicingService;

pub fn merchant_invoicing_routes() -> Router<Arc<MerchantInvoicingService>> {
    Router::new()
        // Tax rules
        .route("/merchant/:merchant_id/invoicing/tax-rules", post(create_tax_rule))
        .route("/merchant/:merchant_id/invoicing/tax-rules", get(list_tax_rules))
        // Tax preview (shown to customer before payment)
        .route("/merchant/:merchant_id/invoicing/tax-preview", post(preview_tax))
        // Invoices
        .route("/merchant/:merchant_id/invoicing/invoices", post(create_invoice))
        .route("/merchant/:merchant_id/invoicing/invoices", get(list_invoices))
        .route("/merchant/:merchant_id/invoicing/invoices/:invoice_id", get(get_invoice))
        // Tax reports (FIRS)
        .route("/merchant/:merchant_id/invoicing/tax-reports", post(generate_tax_report))
        .route("/merchant/:merchant_id/invoicing/tax-reports", get(list_tax_reports))
}
