//! Route registration for Merchant Multi-Sig & Treasury Controls.
//!
//! Mount under `/api/v1` in main.rs.
//!
//! Endpoints:
//!   POST   /merchants/:id/multisig/freeze                        — emergency freeze
//!   DELETE /merchants/:id/multisig/freeze                        — lift freeze
//!   GET    /merchants/:id/multisig/freeze                        — freeze status
//!   POST   /merchants/:id/multisig/policies                      — create signing policy
//!   GET    /merchants/:id/multisig/policies                      — list policies
//!   POST   /merchants/:id/multisig/groups                        — create signing group
//!   POST   /merchants/:id/multisig/groups/:gid/members           — add group member
//!   POST   /merchants/:id/multisig/proposals                     — propose action
//!   GET    /merchants/:id/multisig/proposals                     — list proposals (dashboard)
//!   GET    /merchants/:id/multisig/proposals/:pid                — proposal detail
//!   POST   /merchants/:id/multisig/proposals/:pid/sign           — sign proposal
//!   POST   /merchants/:id/multisig/proposals/:pid/execute        — execute approved proposal

use crate::merchant_multisig::handlers::*;
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn merchant_multisig_routes(state: MultisigState) -> Router {
    Router::new()
        .route("/merchants/:merchant_id/multisig/freeze",
            post(freeze_account).delete(unfreeze_account).get(get_freeze_status))
        .route("/merchants/:merchant_id/multisig/policies",
            post(create_policy).get(list_policies))
        .route("/merchants/:merchant_id/multisig/groups",
            post(create_group))
        .route("/merchants/:merchant_id/multisig/groups/:group_id/members",
            post(add_group_member))
        .route("/merchants/:merchant_id/multisig/proposals",
            post(create_proposal).get(list_proposals))
        .route("/merchants/:merchant_id/multisig/proposals/:proposal_id",
            get(get_proposal))
        .route("/merchants/:merchant_id/multisig/proposals/:proposal_id/sign",
            post(sign_proposal))
        .route("/merchants/:merchant_id/multisig/proposals/:proposal_id/execute",
            post(execute_proposal))
        .with_state(state)
}
