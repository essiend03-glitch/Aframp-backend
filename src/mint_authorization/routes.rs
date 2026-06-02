//! Route registration for the Mint Authorization Framework.

use crate::mint_authorization::handlers::{
    cancel_authorization, create_authorization, get_authorization, list_authorizations,
    sign_authorization, MintAuthState,
};
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes(state: MintAuthState) -> Router {
    Router::new()
        .route(
            "/api/admin/mint/authorizations",
            get(list_authorizations).post(create_authorization),
        )
        .route(
            "/api/admin/mint/authorizations/:auth_id",
            get(get_authorization),
        )
        .route(
            "/api/admin/mint/authorizations/:auth_id/sign",
            post(sign_authorization),
        )
        .route(
            "/api/admin/mint/authorizations/:auth_id/cancel",
            post(cancel_authorization),
        )
        .with_state(state)
}
