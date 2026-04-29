// Automatic Audit Logging Middleware
//
// This module provides middleware that automatically logs all API requests
// and critical system operations to the append-only audit ledger.

use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::Response,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, warn};
use uuid::Uuid;

use super::ledger::{ActionType, ActorType, AuditLedger};

/// Extension for storing audit context in request
#[derive(Clone)]
pub struct AuditContext {
    pub actor_id: String,
    pub actor_type: ActorType,
    pub correlation_id: String,
}

/// Middleware for automatic audit logging
pub async fn audit_logging_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut req: Request,
    next: Next,
) -> Response {
    // Extract audit context from request extensions
    let audit_ctx = req
        .extensions()
        .get::<AuditContext>()
        .cloned()
        .unwrap_or_else(|| AuditContext {
            actor_id: "anonymous".to_string(),
            actor_type: ActorType::External,
            correlation_id: Uuid::new_v4().to_string(),
        });
    
    // Extract request information
    let method = req.method().to_string();
    let uri = req.uri().to_string();
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    
    // Get audit ledger from extensions
    let audit_ledger = req.extensions().get::<Arc<AuditLedger>>().cloned();
    
    // Process request
    let response = next.run(req).await;
    
    // Log to audit ledger (async, don't block response)
    if let Some(ledger) = audit_ledger {
        let status = response.status();
        let result = if status.is_success() {
            "success"
        } else if status.is_client_error() {
            "client_error"
        } else if status.is_server_error() {
            "server_error"
        } else {
            "unknown"
        };
        
        let metadata = serde_json::json!({
            "method": method,
            "uri": uri,
            "status_code": status.as_u16(),
        });
        
        // Spawn async task to log (don't await to avoid blocking)
        let ledger_clone = ledger.clone();
        let audit_ctx_clone = audit_ctx.clone();
        let ip_address = addr.ip().to_string();
        let result_str = result.to_string();
        
        tokio::spawn(async move {
            if let Err(e) = ledger_clone
                .append(
                    audit_ctx_clone.actor_id,
                    audit_ctx_clone.actor_type,
                    ActionType::Execute,
                    None,
                    Some("api_request".to_string()),
                    Some(audit_ctx_clone.correlation_id),
                    metadata,
                    Some(ip_address),
                    user_agent,
                    result_str,
                    None,
                )
                .await
            {
                error!("Failed to log to audit ledger: {}", e);
            }
        });
    } else {
        warn!("Audit ledger not available in request extensions");
    }
    
    response
}

/// Helper to log critical operations
pub struct AuditLogger {
    ledger: Arc<AuditLedger>,
}

impl AuditLogger {
    pub fn new(ledger: Arc<AuditLedger>) -> Self {
        Self { ledger }
    }
    
    /// Log a transaction operation
    pub async fn log_transaction(
        &self,
        actor_id: String,
        actor_type: ActorType,
        action_type: ActionType,
        transaction_id: String,
        amount: String,
        currency: String,
        correlation_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = serde_json::json!({
            "amount": amount,
            "currency": currency,
            "transaction_type": "payment",
        });
        
        self.ledger
            .append(
                actor_id,
                actor_type,
                action_type,
                Some(transaction_id),
                Some("transaction".to_string()),
                correlation_id,
                metadata,
                None,
                None,
                "success".to_string(),
                None,
            )
            .await?;
        
        Ok(())
    }
    
    /// Log a governance action
    pub async fn log_governance(
        &self,
        actor_id: String,
        action_type: ActionType,
        proposal_id: String,
        proposal_type: String,
        correlation_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = serde_json::json!({
            "proposal_type": proposal_type,
        });
        
        self.ledger
            .append(
                actor_id,
                ActorType::Admin,
                action_type,
                Some(proposal_id),
                Some("governance_proposal".to_string()),
                correlation_id,
                metadata,
                None,
                None,
                "success".to_string(),
                None,
            )
            .await?;
        
        Ok(())
    }
    
    /// Log an authentication event
    pub async fn log_authentication(
        &self,
        actor_id: String,
        actor_type: ActorType,
        success: bool,
        ip_address: Option<String>,
        user_agent: Option<String>,
        correlation_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = serde_json::json!({
            "auth_method": "password",
            "success": success,
        });
        
        let result = if success { "success" } else { "failure" };
        let error_message = if !success {
            Some("Authentication failed".to_string())
        } else {
            None
        };
        
        self.ledger
            .append(
                actor_id,
                actor_type,
                ActionType::Authenticate,
                None,
                Some("authentication".to_string()),
                correlation_id,
                metadata,
                ip_address,
                user_agent,
                result.to_string(),
                error_message,
            )
            .await?;
        
        Ok(())
    }
    
    /// Log a mint/burn operation
    pub async fn log_mint_burn(
        &self,
        actor_id: String,
        action_type: ActionType,
        amount: String,
        asset_code: String,
        transaction_id: String,
        correlation_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = serde_json::json!({
            "amount": amount,
            "asset_code": asset_code,
            "operation": if matches!(action_type, ActionType::Mint) { "mint" } else { "burn" },
        });
        
        self.ledger
            .append(
                actor_id,
                ActorType::System,
                action_type,
                Some(transaction_id),
                Some("mint_burn_operation".to_string()),
                correlation_id,
                metadata,
                None,
                None,
                "success".to_string(),
                None,
            )
            .await?;
        
        Ok(())
    }
    
    /// Log a configuration change
    pub async fn log_config_change(
        &self,
        actor_id: String,
        config_key: String,
        old_value: Option<String>,
        new_value: String,
        correlation_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = serde_json::json!({
            "config_key": config_key,
            "old_value": old_value,
            "new_value": new_value,
        });
        
        self.ledger
            .append(
                actor_id,
                ActorType::Admin,
                ActionType::Configure,
                Some(config_key),
                Some("configuration".to_string()),
                correlation_id,
                metadata,
                None,
                None,
                "success".to_string(),
                None,
            )
            .await?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_audit_context_creation() {
        let ctx = AuditContext {
            actor_id: "user123".to_string(),
            actor_type: ActorType::User,
            correlation_id: Uuid::new_v4().to_string(),
        };
        
        assert_eq!(ctx.actor_id, "user123");
        assert!(matches!(ctx.actor_type, ActorType::User));
    }
}
