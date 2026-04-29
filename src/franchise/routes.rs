//! Route definitions for Multi-Store & Franchise Management (Issue #335).

use super::handlers::*;
use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use std::sync::Arc;
use crate::franchise::service::FranchiseService;

pub fn franchise_routes() -> Router<Arc<FranchiseService>> {
    Router::new()
        // Organizations
        .route("/organizations", post(create_organization))
        .route("/organizations/:org_id", get(get_organization))
        .route("/organizations/:org_id/settlement", patch(update_settlement))
        // Regions
        .route("/organizations/:org_id/regions", post(create_region))
        .route("/organizations/:org_id/regions", get(list_regions))
        // Branches
        .route("/organizations/:org_id/branches", post(create_branch))
        .route("/organizations/:org_id/branches", get(list_branches))
        .route("/organizations/:org_id/branches/:branch_id", delete(remove_branch))
        // Members
        .route("/organizations/:org_id/members", post(add_member))
        .route("/organizations/:org_id/members/:user_id", delete(remove_member))
        // Cross-store analytics
        .route("/organizations/:org_id/analytics/revenue", get(cross_store_revenue))
}
