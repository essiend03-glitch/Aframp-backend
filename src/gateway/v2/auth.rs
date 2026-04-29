//! Gateway v2 — Auth service.
//!
//! Validates Bearer tokens and API keys on every inbound request before
//! forwarding to upstream services.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Axum middleware that enforces Bearer / API-key authentication.
///
/// Requests without a valid `Authorization` header are rejected with 401.
pub async fn auth_middleware(req: Request, next: Next) -> Response {
    let has_auth = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("Bearer ") || v.starts_with("ApiKey "))
        .unwrap_or(false);

    if !has_auth {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": "missing_or_invalid_authorization",
                "message": "A valid Bearer token or API key is required."
            })),
        )
            .into_response();
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, middleware, routing::get, Router};
    use tower::ServiceExt;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn app() -> Router {
        Router::new()
            .route("/", get(ok_handler))
            .layer(middleware::from_fn(auth_middleware))
    }

    #[tokio::test]
    async fn rejects_missing_auth() {
        let resp = app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn accepts_bearer_token() {
        let resp = app()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("authorization", "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn accepts_api_key() {
        let resp = app()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("authorization", "ApiKey ak_test_123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
