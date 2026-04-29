use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// W3C-compliant Decentralized Identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DID {
    pub method: String,      // e.g., "stellar", "ethereum"
    pub network: String,     // e.g., "mainnet", "testnet"
    pub identifier: String,  // unique agent identifier
}

impl DID {
    pub fn new(method: &str, network: &str, identifier: &str) -> Self {
        Self {
            method: method.to_string(),
            network: network.to_string(),
            identifier: identifier.to_string(),
        }
    }

    pub fn to_string(&self) -> String {
        format!("did:{}:{}:{}", self.method, self.network, self.identifier)
    }

    pub fn from_string(did_str: &str) -> Result<Self, crate::kya::error::KYAError> {
        let parts: Vec<&str> = did_str.split(':').collect();
        if parts.len() != 4 || parts[0] != "did" {
            return Err(crate::kya::error::KYAError::InvalidDID(did_str.to_string()));
        }
        Ok(Self {
            method: parts[1].to_string(),
            network: parts[2].to_string(),
            identifier: parts[3].to_string(),
        })
    }
}

/// Agent profile containing service endpoints and capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub did: DID,
    pub name: String,
    pub description: Option<String>,
    pub service_endpoints: Vec<ServiceEndpoint>,
    pub capabilities: Vec<Capability>,
    pub owner_address: String,
    pub public_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub id: String,
    pub endpoint_type: String,  // e.g., "API", "RPC", "WebSocket"
    pub url: String,
    pub authentication: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub domain: String,         // e.g., "code_audit", "financial_analysis"
    pub skill_level: u8,        // 1-10
    pub verified: bool,
    pub proof_uri: Option<String>,
}

/// Reputation domains for modular scoring
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReputationDomain {
    CodeAudit,
    FinancialAnalysis,
    ContentCreation,
    DataProcessing,
    SmartContractExecution,
    PaymentProcessing,
    Custom(String),
}

impl ReputationDomain {
    pub fn as_str(&self) -> &str {
        match self {
            Self::CodeAudit => "code_audit",
            Self::FinancialAnalysis => "financial_analysis",
            Self::ContentCreation => "content_creation",
            Self::DataProcessing => "data_processing",
            Self::SmartContractExecution => "smart_contract_execution",
            Self::PaymentProcessing => "payment_processing",
            Self::Custom(s) => s.as_str(),
        }
    }
}

/// Reputation score for a specific domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainReputationScore {
    pub domain: ReputationDomain,
    pub score: f64,              // 0.0 - 100.0
    pub total_interactions: u64,
    pub successful_interactions: u64,
    pub failed_interactions: u64,
    pub last_updated: DateTime<Utc>,
}

/// Cryptographically signed attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationRecord {
    pub id: Uuid,
    pub agent_did: DID,
    pub issuer_did: DID,
    pub domain: ReputationDomain,
    pub claim: String,           // e.g., "Successfully completed 1000 code audits"
    pub evidence_uri: Option<String>,
    pub signature: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Feedback authorization token (prevents Sybil attacks)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackToken {
    pub id: Uuid,
    pub agent_did: DID,
    pub client_did: DID,
    pub interaction_id: Uuid,
    pub domain: ReputationDomain,
    pub authorized_at: DateTime<Utc>,
    pub used: bool,
    pub signature: String,
}

/// Zero-knowledge proof of competence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetenceProofRecord {
    pub id: Uuid,
    pub agent_did: DID,
    pub domain: ReputationDomain,
    pub claim: String,
    pub proof: Vec<u8>,          // ZK proof bytes
    pub public_inputs: Vec<u8>,  // Public parameters
    pub verified: bool,
    pub created_at: DateTime<Utc>,
}

/// Cross-platform reputation verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossPlatformReputation {
    pub agent_did: DID,
    pub source_platform: String,  // e.g., "stellar", "ethereum"
    pub target_platform: String,
    pub reputation_hash: String,
    pub verification_proof: Vec<u8>,
    pub synced_at: DateTime<Utc>,
}
