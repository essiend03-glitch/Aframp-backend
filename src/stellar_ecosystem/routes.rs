//! Routes for Stellar Ecosystem Partner Integration (Issue #470).

#[cfg(feature = "database")]
use crate::stellar_ecosystem::{
    handlers::*,
    service::EcosystemService,
};
#[cfg(feature = "database")]
use axum::{
    routing::{get, post},
    Router,
};
#[cfg(feature = "database")]
use std::sync::Arc;

#[cfg(feature = "database")]
pub fn ecosystem_routes() -> Router<Arc<EcosystemService>> {
    Router::new()
        // Anchor management
        .route(
            "/api/v1/admin/ecosystem/stellar/anchors",
            get(list_anchors_handler).post(register_anchor_handler),
        )
        // DEX configuration
        .route(
            "/api/v1/admin/ecosystem/stellar/dex/configure",
            post(configure_dex_handler),
        )
        // DEX pathfinding
        .route(
            "/api/v1/admin/ecosystem/stellar/dex/path",
            post(find_path_handler),
        )
        // Cross-anchor transfers
        .route(
            "/api/v1/admin/ecosystem/stellar/transfers",
            post(initiate_transfer_handler),
        )
}
