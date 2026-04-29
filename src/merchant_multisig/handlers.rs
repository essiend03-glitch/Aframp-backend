//! HTTP handlers for Merchant Multi-Sig & Treasury Controls.

use crate::merchant_multisig::{
    models::*,
    service::MerchantMultisigService,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

pub type MultisigState = Arc<MerchantMultisigService>;

fn err(code: StatusCode, msg: impl std::fmt::Display) -> axum::response::Response {
    (code, Json(serde_json::json!({ "error": msg.to_string() }))).into_response()
}

fn map_err(e: MultisigError) -> axum::response::Response {
    let code = match &e {
        MultisigError::ProposalNotFound(_) | MultisigError::PolicyNotFound(_) | MultisigError::GroupNotFound(_) => StatusCode::NOT_FOUND,
        MultisigError::AccountFrozen | MultisigError::DuplicateSignature(_) | MultisigError::ProposalNotPending(_) | MultisigError::NotGroupMember(_) => StatusCode::UNPROCESSABLE_ENTITY,
        MultisigError::NoPolicyApplicable(_, _) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    err(code, e)
}

// ── Freeze ────────────────────────────────────────────────────────────────────

/// POST /merchants/:merchant_id/multisig/freeze
pub async fn freeze_account(
    State(svc): State<MultisigState>,
    Path(merchant_id): Path<String>,
    // officer identity from JWT/SSO header set by auth middleware
    axum::extract::Extension(officer_id): axum::extract::Extension<String>,
    Json(req): Json<FreezeRequest>,
) -> impl IntoResponse {
    match svc.freeze(&merchant_id, &officer_id, req).await {
        Ok(state) => (StatusCode::OK, Json(state)).into_response(),
        Err(e) => map_err(e),
    }
}

/// DELETE /merchants/:merchant_id/multisig/freeze
pub async fn unfreeze_account(
    State(svc): State<MultisigState>,
    Path(merchant_id): Path<String>,
    axum::extract::Extension(officer_id): axum::extract::Extension<String>,
    Json(req): Json<UnfreezeRequest>,
) -> impl IntoResponse {
    match svc.unfreeze(&merchant_id, &officer_id, req).await {
        Ok(state) => (StatusCode::OK, Json(state)).into_response(),
        Err(e) => map_err(e),
    }
}

/// GET /merchants/:merchant_id/multisig/freeze
pub async fn get_freeze_status(
    State(svc): State<MultisigState>,
    Path(merchant_id): Path<String>,
) -> impl IntoResponse {
    match svc.get_freeze_state(&merchant_id).await {
        Ok(state) => (StatusCode::OK, Json(state)).into_response(),
        Err(e) => map_err(e),
    }
}

// ── Signing Policies ──────────────────────────────────────────────────────────

/// POST /merchants/:merchant_id/multisig/policies
pub async fn create_policy(
    State(svc): State<MultisigState>,
    Path(merchant_id): Path<String>,
    axum::extract::Extension(creator_id): axum::extract::Extension<String>,
    Json(req): Json<CreateSigningPolicyRequest>,
) -> impl IntoResponse {
    match svc.create_policy(&merchant_id, &creator_id, req).await {
        Ok(p) => (StatusCode::CREATED, Json(p)).into_response(),
        Err(e) => map_err(e),
    }
}

/// GET /merchants/:merchant_id/multisig/policies
pub async fn list_policies(
    State(svc): State<MultisigState>,
    Path(merchant_id): Path<String>,
) -> impl IntoResponse {
    match svc.list_policies(&merchant_id).await {
        Ok(ps) => (StatusCode::OK, Json(ps)).into_response(),
        Err(e) => map_err(e),
    }
}

// ── Signing Groups ────────────────────────────────────────────────────────────

/// POST /merchants/:merchant_id/multisig/groups
pub async fn create_group(
    State(svc): State<MultisigState>,
    Path(merchant_id): Path<String>,
    axum::extract::Extension(creator_id): axum::extract::Extension<String>,
    Json(req): Json<CreateSigningGroupRequest>,
) -> impl IntoResponse {
    match svc.create_group(&merchant_id, &creator_id, req).await {
        Ok(g) => (StatusCode::CREATED, Json(g)).into_response(),
        Err(e) => map_err(e),
    }
}

/// POST /merchants/:merchant_id/multisig/groups/:group_id/members
pub async fn add_group_member(
    State(svc): State<MultisigState>,
    Path((_merchant_id, group_id)): Path<(String, Uuid)>,
    axum::extract::Extension(adder_id): axum::extract::Extension<String>,
    Json(req): Json<AddGroupMemberRequest>,
) -> impl IntoResponse {
    match svc.add_group_member(group_id, &adder_id, req).await {
        Ok(m) => (StatusCode::CREATED, Json(m)).into_response(),
        Err(e) => map_err(e),
    }
}

// ── Proposals ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ProposalQuery {
    pub status: Option<String>,
}

/// POST /merchants/:merchant_id/multisig/proposals
pub async fn create_proposal(
    State(svc): State<MultisigState>,
    Path(merchant_id): Path<String>,
    axum::extract::Extension(proposer_id): axum::extract::Extension<String>,
    Json(req): Json<CreateProposalRequest>,
) -> impl IntoResponse {
    match svc.create_proposal(&merchant_id, &proposer_id, req).await {
        Ok(p) => (StatusCode::CREATED, Json(p)).into_response(),
        Err(e) => map_err(e),
    }
}

/// GET /merchants/:merchant_id/multisig/proposals
pub async fn list_proposals(
    State(svc): State<MultisigState>,
    Path(merchant_id): Path<String>,
    Query(q): Query<ProposalQuery>,
) -> impl IntoResponse {
    match svc.list_proposals(&merchant_id, q.status.as_deref()).await {
        Ok(ps) => (StatusCode::OK, Json(ps)).into_response(),
        Err(e) => map_err(e),
    }
}

/// GET /merchants/:merchant_id/multisig/proposals/:proposal_id
pub async fn get_proposal(
    State(svc): State<MultisigState>,
    Path((_merchant_id, proposal_id)): Path<(String, Uuid)>,
) -> impl IntoResponse {
    match svc.get_proposal(proposal_id).await {
        Ok(p) => (StatusCode::OK, Json(p)).into_response(),
        Err(e) => map_err(e),
    }
}

/// POST /merchants/:merchant_id/multisig/proposals/:proposal_id/sign
pub async fn sign_proposal(
    State(svc): State<MultisigState>,
    Path((_merchant_id, proposal_id)): Path<(String, Uuid)>,
    axum::extract::Extension(signer_id): axum::extract::Extension<String>,
    Json(req): Json<SignProposalRequest>,
) -> impl IntoResponse {
    match svc.sign_proposal(proposal_id, &signer_id, req).await {
        Ok(p) => (StatusCode::OK, Json(p)).into_response(),
        Err(e) => map_err(e),
    }
}

/// POST /merchants/:merchant_id/multisig/proposals/:proposal_id/execute
pub async fn execute_proposal(
    State(svc): State<MultisigState>,
    Path((_merchant_id, proposal_id)): Path<(String, Uuid)>,
    axum::extract::Extension(executor_id): axum::extract::Extension<String>,
) -> impl IntoResponse {
    match svc.execute_proposal(proposal_id, &executor_id).await {
        Ok(p) => (StatusCode::OK, Json(p)).into_response(),
        Err(e) => map_err(e),
    }
}
