//! HTTP handlers for Multi-Store & Franchise Management.

use crate::franchise::{models::*, service::FranchiseService};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub type FranchiseState = Arc<FranchiseService>;

// ---------------------------------------------------------------------------
// Organizations
// ---------------------------------------------------------------------------

/// POST /v1/organizations
/// `owner_user_id` is passed as a query param; in production it comes from JWT middleware.
pub async fn create_organization(
    State(svc): State<FranchiseState>,
    Query(actor): Query<ActorQuery>,
    Json(req): Json<CreateOrganizationRequest>,
) -> impl IntoResponse {
    let owner_id = match actor.actor_user_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "actor_user_id is required" })),
            )
                .into_response()
        }
    };
    match svc.create_organization(owner_id, req).await {
        Ok(org) => (StatusCode::CREATED, Json(json!({ "data": org }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/organizations/:org_id
pub async fn get_organization(
    State(svc): State<FranchiseState>,
    Path(org_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.get_organization(org_id).await {
        Ok(org) => (StatusCode::OK, Json(json!({ "data": org }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// PATCH /v1/organizations/:org_id/settlement
pub async fn update_settlement(
    State(svc): State<FranchiseState>,
    Path(org_id): Path<Uuid>,
    Query(actor): Query<ActorQuery>,
    Json(req): Json<UpdateSettlementRequest>,
) -> impl IntoResponse {
    match svc.update_settlement(org_id, actor.actor_user_id, req).await {
        Ok(org) => (StatusCode::OK, Json(json!({ "data": org }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Regions
// ---------------------------------------------------------------------------

/// POST /v1/organizations/:org_id/regions
pub async fn create_region(
    State(svc): State<FranchiseState>,
    Path(org_id): Path<Uuid>,
    Query(actor): Query<ActorQuery>,
    Json(req): Json<CreateRegionRequest>,
) -> impl IntoResponse {
    match svc.create_region(org_id, actor.actor_user_id, req).await {
        Ok(region) => (StatusCode::CREATED, Json(json!({ "data": region }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/organizations/:org_id/regions
pub async fn list_regions(
    State(svc): State<FranchiseState>,
    Path(org_id): Path<Uuid>,
) -> impl IntoResponse {
    match svc.list_regions(org_id).await {
        Ok(regions) => (StatusCode::OK, Json(json!({ "data": regions }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Branches
// ---------------------------------------------------------------------------

/// POST /v1/organizations/:org_id/branches
pub async fn create_branch(
    State(svc): State<FranchiseState>,
    Path(org_id): Path<Uuid>,
    Query(actor): Query<ActorQuery>,
    Json(req): Json<CreateBranchRequest>,
) -> impl IntoResponse {
    match svc.create_branch(org_id, actor.actor_user_id, req).await {
        Ok(branch) => (StatusCode::CREATED, Json(json!({ "data": branch }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// GET /v1/organizations/:org_id/branches
pub async fn list_branches(
    State(svc): State<FranchiseState>,
    Path(org_id): Path<Uuid>,
    Query(actor): Query<ActorQuery>,
) -> impl IntoResponse {
    match svc.list_branches(org_id, actor.actor_user_id).await {
        Ok(branches) => (StatusCode::OK, Json(json!({ "data": branches }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// DELETE /v1/organizations/:org_id/branches/:branch_id
pub async fn remove_branch(
    State(svc): State<FranchiseState>,
    Path((org_id, branch_id)): Path<(Uuid, Uuid)>,
    Query(actor): Query<ActorQuery>,
) -> impl IntoResponse {
    match svc.remove_branch(org_id, actor.actor_user_id, branch_id).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "message": "Branch deactivated" }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Members
// ---------------------------------------------------------------------------

/// POST /v1/organizations/:org_id/members
pub async fn add_member(
    State(svc): State<FranchiseState>,
    Path(org_id): Path<Uuid>,
    Query(actor): Query<ActorQuery>,
    Json(req): Json<AddMemberRequest>,
) -> impl IntoResponse {
    match svc.add_member(org_id, actor.actor_user_id, req).await {
        Ok(member) => (StatusCode::CREATED, Json(json!({ "data": member }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// DELETE /v1/organizations/:org_id/members/:user_id
pub async fn remove_member(
    State(svc): State<FranchiseState>,
    Path((org_id, user_id)): Path<(Uuid, Uuid)>,
    Query(actor): Query<ActorQuery>,
) -> impl IntoResponse {
    match svc.remove_member(org_id, actor.actor_user_id, user_id).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "message": "Member removed" }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Cross-store analytics
// ---------------------------------------------------------------------------

/// GET /v1/organizations/:org_id/analytics/revenue
pub async fn cross_store_revenue(
    State(svc): State<FranchiseState>,
    Path(org_id): Path<Uuid>,
    Query(params): Query<CrossStoreRevenueParams>,
) -> impl IntoResponse {
    let query = RevenueReportQuery {
        period_start: params.period_start,
        period_end: params.period_end,
        branch_id: params.branch_id,
    };
    match svc.get_cross_store_revenue(org_id, params.actor_user_id, query).await {
        Ok(report) => (StatusCode::OK, Json(json!({ "data": report }))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Shared query params
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct ActorQuery {
    pub actor_user_id: Uuid,
}

#[derive(serde::Deserialize)]
pub struct CrossStoreRevenueParams {
    pub actor_user_id: Uuid,
    pub period_start: chrono::NaiveDate,
    pub period_end: chrono::NaiveDate,
    pub branch_id: Option<Uuid>,
}
