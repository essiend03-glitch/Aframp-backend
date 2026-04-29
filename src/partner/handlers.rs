use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use super::{
    error::PartnerError,
    middleware::PartnerContext,
    models::{ProvisionCredentialRequest, RegisterPartnerRequest},
    service::PartnerService,
};

pub type PartnerState = Arc<PartnerService>;

/// POST /api/partner/v1/register
#[utoipa::path(
    post,
    path = "/api/partner/v1/register",
    tag = "partner",
    request_body = RegisterPartnerRequest,
    responses(
        (status = 201, description = "Partner registered in sandbox"),
        (status = 400, description = "Invalid partner type"),
        (status = 409, description = "Organisation already registered"),
    )
)]
pub async fn register_partner(
    State(svc): State<PartnerState>,
    Json(req): Json<RegisterPartnerRequest>,
) -> Result<impl IntoResponse, PartnerError> {
    let partner = svc.register(req).await?;
    Ok((StatusCode::CREATED, Json(json!({ "partner": partner }))))
}

/// GET /api/partner/v1/partners/:id
#[utoipa::path(
    get,
    path = "/api/partner/v1/partners/{id}",
    tag = "partner",
    params(("id" = Uuid, Path, description = "Partner ID")),
    responses(
        (status = 200, description = "Partner details"),
        (status = 404, description = "Partner not found"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn get_partner(
    State(svc): State<PartnerState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, PartnerError> {
    let partner = svc.get(id).await?;
    Ok(Json(json!({ "partner": partner })))
}

/// POST /api/partner/v1/partners/:id/credentials
#[utoipa::path(
    post,
    path = "/api/partner/v1/partners/{id}/credentials",
    tag = "partner",
    params(("id" = Uuid, Path, description = "Partner ID")),
    request_body = ProvisionCredentialRequest,
    responses(
        (status = 201, description = "Credential provisioned — secret returned once"),
        (status = 400, description = "Invalid credential type"),
        (status = 403, description = "Partner suspended"),
        (status = 404, description = "Partner not found"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn provision_credential(
    State(svc): State<PartnerState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ProvisionCredentialRequest>,
) -> Result<impl IntoResponse, PartnerError> {
    let cred = svc.provision_credential(id, req).await?;
    Ok((StatusCode::CREATED, Json(json!({ "credential": cred }))))
}

/// DELETE /api/partner/v1/partners/:id/credentials/:cred_id
#[utoipa::path(
    delete,
    path = "/api/partner/v1/partners/{id}/credentials/{cred_id}",
    tag = "partner",
    params(
        ("id" = Uuid, Path, description = "Partner ID"),
        ("cred_id" = Uuid, Path, description = "Credential ID"),
    ),
    responses(
        (status = 204, description = "Credential revoked"),
        (status = 404, description = "Credential not found or not owned by partner"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn revoke_credential(
    State(svc): State<PartnerState>,
    Path((id, cred_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, PartnerError> {
    svc.revoke_credential(id, cred_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/partner/v1/partners/:id/validate
#[utoipa::path(
    post,
    path = "/api/partner/v1/partners/{id}/validate",
    tag = "partner",
    params(("id" = Uuid, Path, description = "Partner ID")),
    responses(
        (status = 200, description = "Validation results with certification_ready flag"),
        (status = 404, description = "Partner not found"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn validate_partner(
    State(svc): State<PartnerState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, PartnerError> {
    let results = svc.run_validation(id).await?;
    let all_passed = results.iter().all(|r| r.passed);
    Ok(Json(json!({
        "partner_id": id,
        "certification_ready": all_passed,
        "results": results,
    })))
}

/// POST /api/partner/v1/partners/:id/promote
#[utoipa::path(
    post,
    path = "/api/partner/v1/partners/{id}/promote",
    tag = "partner",
    params(("id" = Uuid, Path, description = "Partner ID")),
    responses(
        (status = 200, description = "Partner promoted to production"),
        (status = 404, description = "Partner not found"),
        (status = 422, description = "Certification tests not all passing"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn promote_to_production(
    State(svc): State<PartnerState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, PartnerError> {
    let partner = svc.promote_to_production(id).await?;
    Ok(Json(json!({ "partner": partner })))
}

/// GET /api/partner/v1/deprecations
#[utoipa::path(
    get,
    path = "/api/partner/v1/deprecations",
    tag = "partner",
    responses(
        (status = 200, description = "List of active API version deprecation notices"),
    )
)]
pub async fn list_deprecations(
    State(svc): State<PartnerState>,
) -> Result<impl IntoResponse, PartnerError> {
    let notices = svc.deprecation_notices().await?;
    Ok(Json(json!({ "deprecations": notices })))
}

/// GET /api/partner/v1/me
#[utoipa::path(
    get,
    path = "/api/partner/v1/me",
    tag = "partner",
    responses(
        (status = 200, description = "Resolved partner identity and auth method"),
        (status = 401, description = "Invalid or missing credential"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn partner_me(
    Extension(ctx): Extension<PartnerContext>,
    State(svc): State<PartnerState>,
) -> Result<impl IntoResponse, PartnerError> {
    let partner = svc.get(ctx.partner_id).await?;
    Ok(Json(json!({
        "partner": partner,
        "auth_method": ctx.auth_method,
        "correlation_id": ctx.correlation_id,
    })))
}
