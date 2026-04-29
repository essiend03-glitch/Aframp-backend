//! Route definitions for Merchant Gateway API

use crate::merchant_gateway::handlers;
use crate::merchant_gateway::service::MerchantGatewayService;
use crate::middleware::api_key::scope_guard;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;

/// Build Merchant Gateway routes with API key authentication
pub fn merchant_gateway_routes(service: Arc<MerchantGatewayService>, pool: Arc<PgPool>) -> Router {
    Router::new()
        // Payment Intent endpoints
        .route("/payment-intents", post(handlers::create_payment_intent))
        .route("/payment-intents", get(handlers::list_payment_intents))
        .route("/payment-intents/:id", get(handlers::get_payment_intent))
        .route(
            "/payment-intents/:id/cancel",
            post(handlers::cancel_payment_intent),
        )
        // Loyalty and cashback campaigns
        .route(
            "/loyalty/campaigns",
            get(handlers::list_loyalty_campaigns).post(handlers::create_loyalty_campaign),
        )
        .route(
            "/loyalty/campaigns/:id/activate",
            post(handlers::activate_loyalty_campaign),
        )
        .route(
            "/loyalty/campaigns/:id/deactivate",
            post(handlers::deactivate_loyalty_campaign),
        )
        .route(
            "/loyalty/reports/spend",
            get(handlers::loyalty_spend_report),
        )
        // Webhook utility
        .route("/webhooks/verify", post(handlers::verify_webhook_signature))
        // Apply API key authentication middleware
        .layer(middleware::from_fn_with_state(
            (pool, "merchant:write", "mainnet"),
            scope_guard,
        ))
        .with_state(service)
}
