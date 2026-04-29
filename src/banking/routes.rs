//! Banking Integration — Route Registration (Issue #407)

use super::handlers::{
    create_mandate, initiate_transfer, link_account, list_accounts, receive_webhook,
    revoke_mandate, unlink_account, BankingState, WebhookState,
};
use axum::{
    routing::{delete, get, post},
    Router,
};

/// User-facing banking routes (require auth middleware in production)
pub fn banking_routes(state: BankingState) -> Router {
    Router::new()
        // Account linkage
        .route("/api/v1/banking/users/{user_id}/accounts", post(link_account))
        .route("/api/v1/banking/users/{user_id}/accounts", get(list_accounts))
        .route(
            "/api/v1/banking/users/{user_id}/accounts/{account_id}",
            delete(unlink_account),
        )
        // Mandates
        .route(
            "/api/v1/banking/users/{user_id}/mandates",
            post(create_mandate),
        )
        .route(
            "/api/v1/banking/users/{user_id}/mandates/{mandate_id}",
            delete(revoke_mandate),
        )
        // Transfers
        .route("/api/v1/banking/transfers", post(initiate_transfer))
        .with_state(state)
}

/// Inbound webhook routes (no auth — validated by HMAC signature in middleware)
pub fn banking_webhook_routes(state: WebhookState) -> Router {
    Router::new()
        .route("/webhooks/banking/{provider}", post(receive_webhook))
        .with_state(state)
}
