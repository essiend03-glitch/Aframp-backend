use axum::{routing::post, Router};
use std::sync::Arc;

use crate::negotiation::handlers::{accept, counter_offer, initiate, NegotiationState};

pub fn router(state: Arc<NegotiationState>) -> Router {
    Router::new()
        .route("/negotiation/initiate", post(initiate))
        .route("/negotiation/counter-offer", post(counter_offer))
        .route("/negotiation/accept", post(accept))
        .with_state(state)
}
