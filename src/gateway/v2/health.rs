//! Gateway v2 — Health check endpoint + structured JSON logging.
//!
//! `GET /gateway/v2/health` returns a JSON body compatible with Kubernetes
//! liveness/readiness probes and modern infrastructure monitoring tools.

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub service: &'static str,
}

/// Handler for `GET /gateway/v2/health`.
pub async fn health_handler() -> impl IntoResponse {
    tracing::info!(
        target: "gateway_v2",
        event = "health_check",
        status = "ok",
        version = "2.0",
        "Gateway v2 health check"
    );

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
            version: "2.0",
            service: "stellar-oxide-gateway",
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_200_with_json_body() {
        let app = Router::new().route("/gateway/v2/health", get(health_handler));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/gateway/v2/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["status"], "ok");
        assert_eq!(json["version"], "2.0");
        assert_eq!(json["service"], "stellar-oxide-gateway");
    }
}
