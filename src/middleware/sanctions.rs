//! Sanctions screening middleware — Issue #419
//!
//! Intercepts every strong-consistency transaction route and runs the sanctions
//! screener **before** the request reaches the handler.
//!
//! Blocked routes (from README endpoint mapping):
//!   /account/*  /api/v1/onramp/*  /api/v1/offramp/*  /api/v1/mint/*
//!   /api/v1/transaction*  /api/v1/transfer*  /api/v1/redemption*
//!
//! Decision logic:
//!   1. Extract `X-Transaction-Id`, `X-Sender-Id`, `X-Sender-Name`,
//!      `X-Receiver-Id`, `X-Receiver-Name` from request headers.
//!   2. If the transaction has an approved bypass → allow through.
//!   3. Run the screener.
//!   4. Append the result to the immutable audit log (best-effort, non-blocking).
//!   5. If outcome is `Hit` or `ProviderError` → return 451 / 503 and halt.
//!   6. Otherwise → forward to the next handler.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::sanctions::{
    AuditLog, BypassService, ScreeningOutcome, ScreeningRequest, SanctionsScreener,
};

/// Shared state injected into the middleware via `axum::Extension`.
#[derive(Clone)]
pub struct SanctionsMiddlewareState {
    pub screener: Arc<SanctionsScreener>,
    pub audit_log: Arc<AuditLog>,
    pub bypass_svc: Arc<BypassService>,
}

/// Paths that require sanctions screening (strong-consistency routes).
const SCREENED_PREFIXES: &[&str] = &[
    "/account/",
    "/api/v1/onramp/",
    "/api/v1/offramp/",
    "/api/v1/mint/",
    "/api/v1/transaction",
    "/api/v1/transfer",
    "/api/v1/redemption",
];

/// Axum middleware function.
pub async fn sanctions_screening_middleware(
    axum::extract::Extension(state): axum::extract::Extension<SanctionsMiddlewareState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();

    // Only screen strong-consistency transaction routes
    if !requires_screening(&path) {
        return next.run(request).await;
    }

    // Extract screening context from headers
    let (transaction_id, screening_req) = match extract_screening_request(request.headers()) {
        Some(r) => r,
        None => {
            // No transaction context in headers — pass through (handler will validate)
            return next.run(request).await;
        }
    };

    // Check for an approved dual-auth bypass
    match state.bypass_svc.is_bypassed(transaction_id).await {
        Ok(true) => {
            info!(
                transaction_id = %transaction_id,
                "Sanctions bypass approved — allowing through"
            );
            return next.run(request).await;
        }
        Ok(false) => {}
        Err(e) => {
            error!(error = %e, "Failed to check bypass status — proceeding with screening");
        }
    }

    // Run the screener
    let result = state.screener.screen(&screening_req).await;

    // Append to immutable audit log (best-effort)
    let audit_log = Arc::clone(&state.audit_log);
    let result_clone = result.clone();
    tokio::spawn(async move {
        if let Err(e) = audit_log.append(&result_clone).await {
            error!(error = %e, "Failed to write sanctions audit log entry");
        }
    });

    match result.outcome {
        ScreeningOutcome::Clear => {
            info!(
                transaction_id = %transaction_id,
                latency_ms = result.latency_ms,
                "Sanctions: clear"
            );
            next.run(request).await
        }

        ScreeningOutcome::Hit => {
            warn!(
                transaction_id = %transaction_id,
                hits = result.matches.len(),
                "Sanctions: HIT — transaction blocked"
            );
            (
                StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, // 451
                axum::Json(json!({
                    "error": "transaction_blocked",
                    "reason": "sanctions_hit",
                    "transaction_id": transaction_id,
                    "status": "BLOCKED_PENDING_REVIEW",
                })),
            )
                .into_response()
        }

        ScreeningOutcome::ProviderError => {
            error!(
                transaction_id = %transaction_id,
                "Sanctions provider error — fail-closed, transaction paused"
            );
            (
                StatusCode::SERVICE_UNAVAILABLE, // 503
                axum::Json(json!({
                    "error": "screening_unavailable",
                    "reason": "provider_error_fail_closed",
                    "transaction_id": transaction_id,
                    "status": "BLOCKED_PENDING_REVIEW",
                })),
            )
                .into_response()
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn requires_screening(path: &str) -> bool {
    SCREENED_PREFIXES.iter().any(|prefix| path.starts_with(prefix))
}

/// Extract a `ScreeningRequest` from request headers.
/// Returns `None` if the required headers are absent (non-transaction requests).
fn extract_screening_request(headers: &HeaderMap) -> Option<(Uuid, ScreeningRequest)> {
    let transaction_id = headers
        .get("x-transaction-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())?;

    let sender_id = header_str(headers, "x-sender-id").unwrap_or("unknown");
    let sender_name = header_str(headers, "x-sender-name").unwrap_or("unknown");
    let receiver_id = header_str(headers, "x-receiver-id").unwrap_or("unknown");
    let receiver_name = header_str(headers, "x-receiver-name").unwrap_or("unknown");
    let intermediary_name = header_str(headers, "x-intermediary-name").map(str::to_owned);

    Some((
        transaction_id,
        ScreeningRequest {
            transaction_id,
            sender_id: sender_id.to_owned(),
            sender_name: sender_name.to_owned(),
            receiver_id: receiver_id.to_owned(),
            receiver_name: receiver_name.to_owned(),
            intermediary_name,
        },
    ))
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}
