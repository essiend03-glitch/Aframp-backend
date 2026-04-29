//! Example route configuration for all CTR endpoints
//!
//! This file shows how to wire up all CTR-related API endpoints in your Axum router.

use crate::aml::{
    ctr_exemption::{CtrExemptionConfig, CtrExemptionService},
    ctr_exemption_handlers::{
        create_exemption, delete_exemption, get_exemptions, get_expiring_exemptions,
        CtrExemptionState,
    },
    ctr_management::{CtrManagementConfig, CtrManagementService},
    ctr_management_handlers::{
        approve_ctr, get_ctr_by_id, get_ctrs, get_ctrs_requiring_senior_approval,
        return_for_correction, review_ctr, CtrManagementState,
    },
};
use axum::{
    routing::{delete, get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;

/// Create all CTR-related routes
///
/// # Example
///
/// ```rust,no_run
/// use axum::Router;
/// use sqlx::PgPool;
///
/// async fn setup_routes(pool: PgPool) -> Router {
///     let ctr_routes = create_all_ctr_routes(pool);
///     
///     Router::new()
///         .nest("/api/admin/compliance", ctr_routes)
/// }
/// ```
pub fn create_all_ctr_routes(pool: PgPool) -> Router {
    // Initialize exemption service
    let exemption_config = CtrExemptionConfig::default();
    let exemption_service = Arc::new(CtrExemptionService::new(pool.clone(), exemption_config));

    // Initialize management service
    let management_config = CtrManagementConfig::default();
    let management_service = Arc::new(CtrManagementService::new(pool.clone(), management_config));

    // Create states
    let exemption_state = CtrExemptionState {
        service: exemption_service,
    };

    let management_state = CtrManagementState {
        service: management_service,
    };

    // Build router with all CTR endpoints
    Router::new()
        // CTR Management endpoints
        .route("/ctrs", get(get_ctrs))
        .route("/ctrs/:ctr_id", get(get_ctr_by_id))
        .route("/ctrs/:ctr_id/review", post(review_ctr))
        .route("/ctrs/:ctr_id/approve", post(approve_ctr))
        .route(
            "/ctrs/:ctr_id/return-for-correction",
            post(return_for_correction),
        )
        .route(
            "/ctrs/senior-approval-required",
            get(get_ctrs_requiring_senior_approval),
        )
        .with_state(management_state)
        // CTR Exemption endpoints
        .route("/ctr/exemptions", post(create_exemption))
        .route("/ctr/exemptions", get(get_exemptions))
        .route("/ctr/exemptions/:exemption_id", delete(delete_exemption))
        .route("/ctr/exemptions/expiring", get(get_expiring_exemptions))
        .with_state(exemption_state)
}

/// Create CTR management routes only
pub fn create_ctr_management_routes(pool: PgPool) -> Router {
    let config = CtrManagementConfig::default();
    let service = Arc::new(CtrManagementService::new(pool, config));

    let state = CtrManagementState { service };

    Router::new()
        .route("/ctrs", get(get_ctrs))
        .route("/ctrs/:ctr_id", get(get_ctr_by_id))
        .route("/ctrs/:ctr_id/review", post(review_ctr))
        .route("/ctrs/:ctr_id/approve", post(approve_ctr))
        .route(
            "/ctrs/:ctr_id/return-for-correction",
            post(return_for_correction),
        )
        .route(
            "/ctrs/senior-approval-required",
            get(get_ctrs_requiring_senior_approval),
        )
        .with_state(state)
}

/// Full integration example with all CTR services
pub fn create_full_ctr_system(pool: PgPool) -> Router {
    use crate::aml::{
        ctr_aggregation::{CtrAggregationConfig, CtrAggregationService},
        ctr_generator::{CtrGeneratorConfig, CtrGeneratorService},
    };

    // Initialize exemption service
    let exemption_config = CtrExemptionConfig::default();
    let exemption_service = Arc::new(CtrExemptionService::new(pool.clone(), exemption_config));

    // Initialize CTR generator with exemption checking
    let generator_config = CtrGeneratorConfig::default();
    let ctr_generator = Arc::new(CtrGeneratorService::with_exemption_service(
        pool.clone(),
        generator_config,
        exemption_service.clone(),
    ));

    // Initialize aggregation service with CTR generation
    let aggregation_config = CtrAggregationConfig::default();
    let _aggregation_service = CtrAggregationService::with_ctr_generator(
        pool.clone(),
        aggregation_config,
        ctr_generator,
    );

    // Initialize management service
    let management_config = CtrManagementConfig::default();
    let management_service = Arc::new(CtrManagementService::new(pool.clone(), management_config));

    // Create states
    let exemption_state = CtrExemptionState {
        service: exemption_service,
    };

    let management_state = CtrManagementState {
        service: management_service,
    };

    // Build complete router
    Router::new()
        // Management endpoints
        .route("/ctrs", get(get_ctrs))
        .route("/ctrs/:ctr_id", get(get_ctr_by_id))
        .route("/ctrs/:ctr_id/review", post(review_ctr))
        .route("/ctrs/:ctr_id/approve", post(approve_ctr))
        .route(
            "/ctrs/:ctr_id/return-for-correction",
            post(return_for_correction),
        )
        .route(
            "/ctrs/senior-approval-required",
            get(get_ctrs_requiring_senior_approval),
        )
        .with_state(management_state)
        // Exemption endpoints
        .route("/ctr/exemptions", post(create_exemption))
        .route("/ctr/exemptions", get(get_exemptions))
        .route("/ctr/exemptions/:exemption_id", delete(delete_exemption))
        .route("/ctr/exemptions/expiring", get(get_expiring_exemptions))
        .with_state(exemption_state)
}

/// Custom configuration example
pub fn create_ctr_routes_with_custom_config(pool: PgPool) -> Router {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    // Custom management config with higher threshold
    let management_config = CtrManagementConfig {
        senior_approval_threshold: Decimal::from_str("100000000").unwrap(), // NGN 100M
        enforce_checklist: true,
    };

    let management_service = Arc::new(CtrManagementService::new(pool, management_config));

    let state = CtrManagementState {
        service: management_service,
    };

    Router::new()
        .route("/ctrs", get(get_ctrs))
        .route("/ctrs/:ctr_id", get(get_ctr_by_id))
        .route("/ctrs/:ctr_id/review", post(review_ctr))
        .route("/ctrs/:ctr_id/approve", post(approve_ctr))
        .route(
            "/ctrs/:ctr_id/return-for-correction",
            post(return_for_correction),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_route_creation() {
        let pool = PgPool::connect("postgresql://localhost/test")
            .await
            .unwrap();
        let _router = create_all_ctr_routes(pool);
        // Router created successfully
    }
}
