//! Route definitions for Wallet Provisioning (Issue #322).

use super::handlers::*;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use crate::wallet_provisioning::service::WalletProvisioningService;

pub fn wallet_provisioning_routes() -> Router<Arc<WalletProvisioningService>> {
    Router::new()
        // Keypair guidance
        .route("/wallet/keypair-guidance", get(get_keypair_guidance))
        .route("/wallet/mnemonic-challenge", get(get_mnemonic_challenge))
        // Funding requirements
        .route("/wallet/:wallet_id/funding-requirements", get(get_funding_requirements))
        // Provisioning status (resumable)
        .route("/wallet/:wallet_id/provisioning-status", get(get_provisioning_status))
        // Trustline
        .route("/wallet/:wallet_id/trustline/initiate", post(initiate_trustline))
        .route("/wallet/:wallet_id/trustline/submit", post(submit_trustline))
        // Readiness
        .route("/wallet/:wallet_id/readiness", get(get_readiness))
        // Admin
        .route("/admin/wallet/funding-account", get(get_funding_account))
        .route("/admin/wallet/funding-account/replenish", post(replenish_funding_account))
}
