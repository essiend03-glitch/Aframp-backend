//! Partner Hub middleware — per-partner rate limiting, auth enforcement,
//! IP whitelist enforcement, and audit logging with Partner_ID + Correlation_ID.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::audit::{
    models::{AuditActorType, AuditEventCategory, AuditOutcome, PendingAuditEntry},
    writer::AuditWriter,
};

use super::{error::PartnerError, repository::PartnerRepository, service::PartnerService};

/// Axum extension injected by the middleware so handlers can read partner context.
#[derive(Clone, Debug)]
pub struct PartnerContext {
    pub partner_id: Uuid,
    pub correlation_id: Uuid,
    pub auth_method: String, // "oauth2" | "mtls" | "api_key"
}

/// Shared state for the partner middleware.
#[derive(Clone)]
pub struct PartnerMiddlewareState {
    pub service: Arc<PartnerService>,
    pub repo: Arc<PartnerRepository>,
    pub audit: Option<Arc<AuditWriter>>,
}

/// Enforce partner authentication (OAuth2 Bearer, mTLS cert fingerprint, or API key),
/// IP whitelist, per-partner rate limiting, and inject audit context headers.
pub async fn partner_auth_middleware(
    State(state): State<Arc<PartnerMiddlewareState>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let correlation_id = Uuid::new_v4();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    // Extract client IP from X-Forwarded-For or X-Real-IP
    let client_ip = extract_client_ip(req.headers());

    // Resolve partner_id + auth_method from request credentials
    let auth_result = resolve_partner(&state.repo, req.headers()).await;

    let (partner_id, auth_method) = match auth_result {
        Ok(v) => v,
        Err(e) => {
            emit_audit(
                &state,
                None,
                client_ip.as_deref(),
                &method,
                &path,
                StatusCode::UNAUTHORIZED.as_u16() as i32,
                start.elapsed().as_millis() as i64,
                AuditOutcome::Failure,
                Some(e.to_string()),
            )
            .await;
            return e.into_response();
        }
    };

    // IP whitelist enforcement
    if let Err(e) = check_ip_whitelist(&state.repo, partner_id, client_ip.as_deref()).await {
        emit_audit(
            &state,
            Some(partner_id),
            client_ip.as_deref(),
            &method,
            &path,
            StatusCode::FORBIDDEN.as_u16() as i32,
            start.elapsed().as_millis() as i64,
            AuditOutcome::Failure,
            Some(e.to_string()),
        )
        .await;
        return e.into_response();
    }

    // Per-partner rate limit
    if let Err(e) = state.service.check_rate_limit(partner_id).await {
        emit_audit(
            &state,
            Some(partner_id),
            client_ip.as_deref(),
            &method,
            &path,
            StatusCode::TOO_MANY_REQUESTS.as_u16() as i32,
            start.elapsed().as_millis() as i64,
            AuditOutcome::Failure,
            Some(e.to_string()),
        )
        .await;
        return e.into_response();
    }

    // Inject partner context as extension
    req.extensions_mut().insert(PartnerContext {
        partner_id,
        correlation_id,
        auth_method: auth_method.clone(),
    });

    // Propagate Partner-ID and Correlation-ID headers downstream
    req.headers_mut().insert(
        "x-partner-id",
        HeaderValue::from_str(&partner_id.to_string()).unwrap(),
    );
    req.headers_mut().insert(
        "x-correlation-id",
        HeaderValue::from_str(&correlation_id.to_string()).unwrap(),
    );

    let mut resp = next.run(req).await;
    let status = resp.status().as_u16() as i32;
    let latency_ms = start.elapsed().as_millis() as i64;

    // Echo correlation ID on response for client tracing
    resp.headers_mut().insert(
        "x-correlation-id",
        HeaderValue::from_str(&correlation_id.to_string()).unwrap(),
    );

    // Emit audit entry
    let outcome = if status < 400 {
        AuditOutcome::Success
    } else {
        AuditOutcome::Failure
    };
    emit_audit(
        &state,
        Some(partner_id),
        client_ip.as_deref(),
        &method,
        &path,
        status,
        latency_ms,
        outcome,
        None,
    )
    .await;

    resp
}

/// Resolve partner identity from the request headers.
/// Priority: mTLS cert fingerprint → OAuth2 Bearer → API key.
async fn resolve_partner(
    repo: &PartnerRepository,
    headers: &HeaderMap,
) -> Result<(Uuid, String), PartnerError> {
    // 1. mTLS — client cert fingerprint forwarded by the TLS terminator
    if let Some(fp) = headers
        .get("x-client-cert-fingerprint")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(cred) = repo.find_credential_by_cert_fingerprint(fp).await? {
            check_credential_validity(&cred)?;
            return Ok((cred.partner_id, "mtls".to_string()));
        }
    }

    // 2. OAuth2 Bearer token — extract client_id from token prefix
    if let Some(bearer) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        // Tokens are formatted as "<client_id>.<secret>" — extract client_id
        if let Some(client_id) = bearer.split('.').next() {
            if let Some(cred) = repo.find_credential_by_client_id(client_id).await? {
                check_credential_validity(&cred)?;
                return Ok((cred.partner_id, "oauth2".to_string()));
            }
        }
    }

    // 3. API key — prefix lookup
    if let Some(key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let prefix = key.split('.').next().unwrap_or("");
        if let Some(cred) = repo.find_credential_by_api_key_prefix(prefix).await? {
            check_credential_validity(&cred)?;
            // Verify full key hash
            let provided_hash = sha256_hex(key);
            if cred.api_key_hash.as_deref() != Some(&provided_hash) {
                return Err(PartnerError::CredentialNotFound);
            }
            return Ok((cred.partner_id, "api_key".to_string()));
        }
    }

    Err(PartnerError::CredentialNotFound)
}

/// Enforce IP whitelist: if the partner has a non-empty whitelist, the client IP
/// must appear in it.
async fn check_ip_whitelist(
    repo: &PartnerRepository,
    partner_id: Uuid,
    client_ip: Option<&str>,
) -> Result<(), PartnerError> {
    let partner = repo.find_by_id(partner_id).await?;
    if partner.ip_whitelist.is_empty() {
        return Ok(()); // no whitelist configured — allow all
    }
    let ip = client_ip.unwrap_or("");
    if partner.ip_whitelist.iter().any(|allowed| allowed == ip) {
        Ok(())
    } else {
        Err(PartnerError::IpNotWhitelisted)
    }
}

fn check_credential_validity(
    cred: &super::models::PartnerCredential,
) -> Result<(), PartnerError> {
    if cred.revoked_at.is_some() {
        return Err(PartnerError::CredentialRevoked);
    }
    if let Some(exp) = cred.expires_at {
        if exp < chrono::Utc::now() {
            return Err(PartnerError::CredentialExpired);
        }
    }
    Ok(())
}

fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
}

#[allow(clippy::too_many_arguments)]
async fn emit_audit(
    state: &PartnerMiddlewareState,
    partner_id: Option<Uuid>,
    client_ip: Option<&str>,
    method: &str,
    path: &str,
    status: i32,
    latency_ms: i64,
    outcome: AuditOutcome,
    failure_reason: Option<String>,
) {
    let Some(audit) = &state.audit else { return };
    let entry = PendingAuditEntry {
        event_type: format!("partner.{}", method.to_lowercase()),
        event_category: AuditEventCategory::Credential,
        actor_type: AuditActorType::Microservice,
        actor_id: partner_id.map(|id| id.to_string()),
        actor_ip: client_ip.map(|s| s.to_string()),
        actor_consumer_type: Some("partner".to_string()),
        session_id: None,
        target_resource_type: Some("partner_api".to_string()),
        target_resource_id: partner_id.map(|id| id.to_string()),
        request_method: method.to_string(),
        request_path: path.to_string(),
        request_body_hash: None,
        response_status: status,
        response_latency_ms: latency_ms,
        outcome,
        failure_reason,
        environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
    };
    audit.write(entry).await;
}

/// Middleware that injects a `Deprecation` response header when the partner's
/// API version is scheduled for sunset.
pub async fn deprecation_header_middleware(
    State(state): State<Arc<PartnerMiddlewareState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    // Extract version from path prefix, e.g. /api/partner/v1/...
    let version = req
        .uri()
        .path()
        .split('/')
        .find(|s| s.starts_with('v') && s.len() > 1)
        .unwrap_or("v1")
        .to_string();

    let mut resp = next.run(req).await;

    if let Ok(Some(dep)) = state.repo.deprecation_for_version(&version).await {
        if let Ok(v) = HeaderValue::from_str(&dep.sunset_at.to_rfc2822()) {
            resp.headers_mut().insert("sunset", v);
        }
        if let Ok(v) = HeaderValue::from_str(&dep.deprecated_at.to_rfc2822()) {
            resp.headers_mut().insert("deprecation", v);
        }
        if let Some(url) = &dep.migration_guide_url {
            if let Ok(v) =
                HeaderValue::from_str(&format!(r#"<{}>; rel="successor-version""#, url))
            {
                resp.headers_mut().insert("link", v);
            }
        }
    }

    resp
}
