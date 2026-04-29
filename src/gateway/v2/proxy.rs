//! Gateway v2 — Proxy service.
//!
//! Strips spoofable inbound headers and injects gateway-identity headers
//! before forwarding requests to upstream services.

use axum::http::HeaderMap;

/// Headers that clients must not be allowed to spoof into upstream services.
const SPOOFABLE_HEADERS: &[&str] = &[
    "x-consumer-id",
    "x-service-name",
    "x-gateway-verified",
    "x-internal-token",
];

/// Strip spoofable headers from an inbound request header map.
pub fn strip_spoofable_headers(headers: &mut HeaderMap) {
    for name in SPOOFABLE_HEADERS {
        headers.remove(*name);
    }
}

/// Inject gateway-identity headers so upstream services can verify the
/// request passed through the v2 gateway.
pub fn inject_v2_gateway_headers(headers: &mut HeaderMap) {
    headers.insert(
        "x-gateway-version",
        axum::http::HeaderValue::from_static("2.0"),
    );
    headers.insert(
        "x-gateway-verified",
        axum::http::HeaderValue::from_static("true"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn strips_spoofable_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-consumer-id", HeaderValue::from_static("spoofed"));
        headers.insert("x-service-name", HeaderValue::from_static("fake"));
        headers.insert("authorization", HeaderValue::from_static("Bearer tok"));

        strip_spoofable_headers(&mut headers);

        assert!(headers.get("x-consumer-id").is_none());
        assert!(headers.get("x-service-name").is_none());
        assert!(headers.get("authorization").is_some(), "auth header must be preserved");
    }

    #[test]
    fn injects_gateway_identity_headers() {
        let mut headers = HeaderMap::new();
        inject_v2_gateway_headers(&mut headers);

        assert_eq!(headers["x-gateway-version"], "2.0");
        assert_eq!(headers["x-gateway-verified"], "true");
    }
}
