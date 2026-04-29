//! Example route configuration for CTR exemption endpoints
//!
//! This file shows how to wire up the CTR exemption API endpoints in your Axum router.

use crate::aml::{
    ctr_exemption::{CtrExemptionConfig, CtrExemptionService},
    ctr_exemption_handlers::{
        create_exemption, delete_exemption, get_exemptions, get_expiring_exemptions,
        CtrExemptionState,
    },
};
use axum::{
    routing::{delete, get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;

/// Create the CTR exemption routes
///
/// # Example
///
/// ```rust,no_run
/// use axum::Router;
/// use sqlx::PgPool;
///
/// async fn setup_routes(pool: PgPool) -> Router {
///     let exemption_routes = create_ctr_exemption_routes(pool);
///     
///     Router::new()
///         .nest("/api/admin/compliance/ctr", exemption_routes)
/// }
/// ```
pub fn create_ctr_exemption_routes(pool: PgPool) -> Router {
    // Initialize exemption service
    let exemption_config = CtrExemptionConfig::default();
    let exemption_service = Arc::new(CtrExemptionService::new(pool, exemption_config));

    // Create shared state
    let state = CtrExemptionState {
        service: exemption_service,
    };

    // Build router with all exemption endpoints
    Router::new()
        .route("/exemptions", post(create_exemption))
        .route("/exemptions", get(get_exemptions))
        .route("/exemptions/:exemption_id", delete(delete_exemption))
        .route("/exemptions/expiring", get(get_expiring_exemptions))
        .with_state(state)
}

/// Full integration example with CTR generator and aggregation
///
/// This shows how to wire up all CTR-related services together
pub fn create_full_ctr_routes(pool: PgPool) -> Router {
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

    // Create exemption state
    let exemption_state = CtrExemptionState {
        service: exemption_service,
    };

    // Build router
    Router::new()
        .route("/exemptions", post(create_exemption))
        .route("/exemptions", get(get_exemptions))
        .route("/exemptions/:exemption_id", delete(delete_exemption))
        .route("/exemptions/expiring", get(get_expiring_exemptions))
        .with_state(exemption_state)
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
        let _router = create_ctr_exemption_routes(pool);
        // Router created successfully
    }
}
