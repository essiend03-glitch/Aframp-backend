use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;

use crate::audit::writer::AuditWriter;

use super::{
    handlers::{
        get_partner, list_deprecations, partner_me, promote_to_production, provision_credential,
        register_partner, revoke_credential, validate_partner,
    },
    middleware::{deprecation_header_middleware, partner_auth_middleware, PartnerMiddlewareState},
    repository::PartnerRepository,
    service::PartnerService,
};

/// Build the Partner Hub router.
///
/// Public routes (no auth):
///   POST /api/partner/v1/register
///   GET  /api/partner/v1/deprecations
///
/// Authenticated routes (require partner credential):
///   GET    /api/partner/v1/me
///   GET    /api/partner/v1/partners/:id
///   POST   /api/partner/v1/partners/:id/credentials
///   DELETE /api/partner/v1/partners/:id/credentials/:cred_id
///   POST   /api/partner/v1/partners/:id/validate
///   POST   /api/partner/v1/partners/:id/promote
pub fn partner_routes(pool: PgPool, audit: Option<Arc<AuditWriter>>) -> Router {
    let repo = PartnerRepository::new(pool.clone());
    let svc = Arc::new(PartnerService::new(repo.clone()));
    let mw_state = Arc::new(PartnerMiddlewareState {
        service: svc.clone(),
        repo: Arc::new(repo),
        audit,
    });

    // Public — no auth required
    let public = Router::new()
        .route("/register", post(register_partner))
        .route("/deprecations", get(list_deprecations))
        .with_state(svc.clone());

    // Authenticated — partner credential required
    let authenticated = Router::new()
        .route("/me", get(partner_me))
        .route("/partners/:id", get(get_partner))
        .route("/partners/:id/credentials", post(provision_credential))
        .route(
            "/partners/:id/credentials/:cred_id",
            delete(revoke_credential),
        )
        .route("/partners/:id/validate", post(validate_partner))
        .route("/partners/:id/promote", post(promote_to_production))
        .layer(middleware::from_fn_with_state(
            mw_state.clone(),
            partner_auth_middleware,
        ))
        .with_state(svc);

    Router::new()
        .nest("/api/partner/v1", public.merge(authenticated))
        .layer(middleware::from_fn_with_state(
            mw_state,
            deprecation_header_middleware,
        ))
}
