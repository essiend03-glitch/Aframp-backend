//! Edge-cache response middleware (Issue #348).
//!
//! Applies path-based Cache-Control headers and enforces consistency routing:
//!   - `/public/*`  → aggressive TTL + stale-while-revalidate (eventual consistency)
//!   - `/account/*` → no-store / private (strong consistency)
//!   - Everything else → short TTL, private
//!
//! Consistency header: `X-Consistency: strong` forces routing to the primary region
//! by setting `X-Route-Primary: true`, which the upstream load balancer honours.

use axum::{
    body::Body,
    http::{Request, Response},
    middleware::Next,
};

const PUBLIC_TTL: u32 = 300;          // 5 min
const PUBLIC_SWR: u32 = 60;           // stale-while-revalidate window
const DEFAULT_TTL: u32 = 30;

/// Axum middleware: attach Cache-Control headers and consistency routing signal.
pub async fn edge_cache_middleware(req: Request<Body>, next: Next) -> Response<Body> {
    let path = req.uri().path().to_owned();
    let wants_strong = req
        .headers()
        .get("x-consistency")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("strong"))
        .unwrap_or(false);

    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();

    // Consistency routing signal for the load balancer / edge proxy.
    if wants_strong {
        headers.insert(
            "x-route-primary",
            "true".parse().expect("static header value"),
        );
    }

    // Path-based Cache-Control strategy.
    let cache_control = if path.starts_with("/public/") {
        format!(
            "public, max-age={PUBLIC_TTL}, stale-while-revalidate={PUBLIC_SWR}"
        )
    } else if path.starts_with("/account/") {
        "no-store, private".to_owned()
    } else {
        format!("private, max-age={DEFAULT_TTL}")
    };

    if let Ok(val) = cache_control.parse() {
        headers.insert("cache-control", val);
    }

    // Vary header so CDN caches are keyed correctly.
    headers.insert(
        "vary",
        "Accept-Encoding, X-Consistency".parse().expect("static header value"),
    );

    resp
}
