use serde::{Deserialize, Serialize};

/// Consistency level for API endpoints
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsistencyLevel {
    /// Eventual consistency - can serve stale data from edge/replicas
    /// Used for: public ledger snapshots, token metadata, historical data
    Eventual,
    
    /// Strong consistency - must route to primary region
    /// Used for: transaction signing, account state, balance queries
    Strong,
    
    /// Read-after-write consistency - serves from primary after write
    /// Used for: user-specific data, recent transactions
    ReadAfterWrite,
}

impl ConsistencyLevel {
    pub fn can_use_edge_cache(&self) -> bool {
        matches!(self, ConsistencyLevel::Eventual)
    }

    pub fn can_use_replica(&self) -> bool {
        matches!(self, ConsistencyLevel::Eventual | ConsistencyLevel::ReadAfterWrite)
    }

    pub fn requires_primary(&self) -> bool {
        matches!(self, ConsistencyLevel::Strong)
    }

    pub fn description(&self) -> &'static str {
        match self {
            ConsistencyLevel::Eventual => "Eventual consistency - may serve stale data",
            ConsistencyLevel::Strong => "Strong consistency - always fresh from primary",
            ConsistencyLevel::ReadAfterWrite => "Read-after-write consistency",
        }
    }
}

/// Endpoint consistency policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointPolicy {
    pub path: String,
    pub consistency: ConsistencyLevel,
    pub cache_ttl_seconds: Option<u64>,
    pub description: String,
}

impl EndpointPolicy {
    pub fn new(path: String, consistency: ConsistencyLevel, description: String) -> Self {
        let cache_ttl_seconds = match consistency {
            ConsistencyLevel::Eventual => Some(300),      // 5 minutes
            ConsistencyLevel::ReadAfterWrite => Some(60), // 1 minute
            ConsistencyLevel::Strong => None,
        };

        Self {
            path,
            consistency,
            cache_ttl_seconds,
            description,
        }
    }
}

/// Consistency manager for routing decisions
pub struct ConsistencyManager {
    policies: Vec<EndpointPolicy>,
}

impl ConsistencyManager {
    pub fn new() -> Self {
        Self {
            policies: Self::default_policies(),
        }
    }

    fn default_policies() -> Vec<EndpointPolicy> {
        vec![
            // Public data endpoints - eventual consistency
            EndpointPolicy::new(
                "/api/v1/ledger/snapshots".to_string(),
                ConsistencyLevel::Eventual,
                "Public ledger snapshots - cacheable".to_string(),
            ),
            EndpointPolicy::new(
                "/api/v1/tokens/metadata".to_string(),
                ConsistencyLevel::Eventual,
                "Token metadata - cacheable".to_string(),
            ),
            EndpointPolicy::new(
                "/api/v1/rates".to_string(),
                ConsistencyLevel::Eventual,
                "Exchange rates - cacheable".to_string(),
            ),
            EndpointPolicy::new(
                "/api/v1/transactions/history".to_string(),
                ConsistencyLevel::Eventual,
                "Historical transactions - cacheable".to_string(),
            ),
            
            // User-specific data - read-after-write
            EndpointPolicy::new(
                "/api/v1/accounts/balance".to_string(),
                ConsistencyLevel::ReadAfterWrite,
                "Account balance - read-after-write".to_string(),
            ),
            EndpointPolicy::new(
                "/api/v1/transactions/recent".to_string(),
                ConsistencyLevel::ReadAfterWrite,
                "Recent transactions - read-after-write".to_string(),
            ),
            
            // Critical operations - strong consistency
            EndpointPolicy::new(
                "/api/v1/transactions/sign".to_string(),
                ConsistencyLevel::Strong,
                "Transaction signing - must use primary".to_string(),
            ),
            EndpointPolicy::new(
                "/api/v1/accounts/create".to_string(),
                ConsistencyLevel::Strong,
                "Account creation - must use primary".to_string(),
            ),
            EndpointPolicy::new(
                "/api/v1/keys/rotate".to_string(),
                ConsistencyLevel::Strong,
                "Key rotation - must use primary".to_string(),
            ),
        ]
    }

    pub fn get_policy(&self, path: &str) -> Option<&EndpointPolicy> {
        self.policies.iter()
            .find(|p| p.path == path || path.starts_with(&p.path))
    }

    pub fn add_policy(&mut self, policy: EndpointPolicy) {
        self.policies.push(policy);
    }

    pub fn list_policies(&self) -> &[EndpointPolicy] {
        &self.policies
    }
}

impl Default for ConsistencyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistency_level_properties() {
        assert!(ConsistencyLevel::Eventual.can_use_edge_cache());
        assert!(!ConsistencyLevel::Strong.can_use_edge_cache());
        
        assert!(ConsistencyLevel::Strong.requires_primary());
        assert!(!ConsistencyLevel::Eventual.requires_primary());
    }

    #[test]
    fn test_consistency_manager_policies() {
        let manager = ConsistencyManager::new();
        
        let ledger_policy = manager.get_policy("/api/v1/ledger/snapshots");
        assert!(ledger_policy.is_some());
        assert_eq!(ledger_policy.unwrap().consistency, ConsistencyLevel::Eventual);
        
        let sign_policy = manager.get_policy("/api/v1/transactions/sign");
        assert!(sign_policy.is_some());
        assert_eq!(sign_policy.unwrap().consistency, ConsistencyLevel::Strong);
    }
}
