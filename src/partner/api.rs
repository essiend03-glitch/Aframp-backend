/// Represents data access scopes granted by the user
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessScope {
    ReadTransactionHistory,
    WritePaymentInitiation,
    ReadBalance,
    ReadKyc, // Strictly restricted
}

/// User consent structure
#[derive(Debug, Clone)]
pub struct UserConsent {
    pub user_id: String,
    pub partner_id: String,
    pub granted_scopes: Vec<AccessScope>,
    pub expires_at: u64,
}

impl UserConsent {
    pub fn has_scope(&self, scope: &AccessScope) -> bool {
        self.granted_scopes.contains(scope)
    }

    pub fn revoke(&mut self) {
        self.granted_scopes.clear();
    }
}

/// Webhook Event Types
#[derive(Debug, Clone)]
pub enum WebhookEvent {
    BalanceChanged { account_id: String, new_balance: f64 },
    TransactionCompleted { tx_id: String, status: String },
}

/// Subscriber trait for third-party webhooks
pub trait EventSubscription {
    /// Dispatches an event payload to a partner's webhook URL
    fn dispatch_event(&self, partner_id: &str, event: WebhookEvent) -> Result<(), String>;
}

/// GraphQL structure placeholders
pub mod graphql_api {
    /// Resolves basic partner-specific analytics
    pub fn resolve_partner_analytics(partner_id: &str) -> String {
        // Mock JSON response
        format!("{{ \"partner\": \"{}\", \"volume_generated\": 150000.00 }}", partner_id)
    }
}
