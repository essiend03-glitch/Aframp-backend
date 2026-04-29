use crate::kya::{error::KYAError, models::*};
use chrono::Utc;
use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Agent identity with cryptographic key pair
#[derive(Debug, Clone)]
pub struct AgentIdentity {
    pub profile: AgentProfile,
    keypair: Option<Keypair>,  // Private key only stored locally
}

impl AgentIdentity {
    /// Create a new agent identity with generated keypair
    pub fn new(
        method: &str,
        network: &str,
        name: String,
        owner_address: String,
    ) -> Result<Self, KYAError> {
        let mut csprng = OsRng {};
        let keypair = Keypair::generate(&mut csprng);
        let public_key = hex::encode(keypair.public.as_bytes());
        
        let identifier = hex::encode(&keypair.public.as_bytes()[..16]);
        let did = DID::new(method, network, &identifier);

        let profile = AgentProfile {
            did: did.clone(),
            name,
            description: None,
            service_endpoints: Vec::new(),
            capabilities: Vec::new(),
            owner_address,
            public_key,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        Ok(Self {
            profile,
            keypair: Some(keypair),
        })
    }

    /// Load existing identity from profile (without private key)
    pub fn from_profile(profile: AgentProfile) -> Self {
        Self {
            profile,
            keypair: None,
        }
    }

    /// Sign a message with the agent's private key
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, KYAError> {
        let keypair = self.keypair.as_ref()
            .ok_or(KYAError::CryptoError("Private key not available".to_string()))?;
        
        let signature = keypair.sign(message);
        Ok(signature.to_bytes().to_vec())
    }

    /// Verify a signature against this agent's public key
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, KYAError> {
        let public_key_bytes = hex::decode(&self.profile.public_key)
            .map_err(|e| KYAError::CryptoError(e.to_string()))?;
        
        let public_key = PublicKey::from_bytes(&public_key_bytes)
            .map_err(|e| KYAError::CryptoError(e.to_string()))?;
        
        let sig = Signature::from_bytes(signature)
            .map_err(|e| KYAError::CryptoError(e.to_string()))?;
        
        Ok(public_key.verify(message, &sig).is_ok())
    }

    /// Export public profile (safe to share)
    pub fn export_profile(&self) -> AgentProfile {
        self.profile.clone()
    }
}

/// Identity registry for managing agent identities on-chain
pub struct IdentityRegistry {
    pool: PgPool,
}

impl IdentityRegistry {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Register a new agent identity
    pub async fn register(&self, identity: &AgentIdentity) -> Result<(), KYAError> {
        let profile = &identity.profile;
        let capabilities_json = serde_json::to_value(&profile.capabilities)?;
        let endpoints_json = serde_json::to_value(&profile.service_endpoints)?;

        sqlx::query!(
            r#"
            INSERT INTO kya_agent_identities 
            (did, method, network, identifier, name, description, owner_address, 
             public_key, capabilities, service_endpoints, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
            profile.did.to_string(),
            profile.did.method,
            profile.did.network,
            profile.did.identifier,
            profile.name,
            profile.description,
            profile.owner_address,
            profile.public_key,
            capabilities_json,
            endpoints_json,
            profile.created_at,
            profile.updated_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieve an agent identity by DID
    pub async fn get_by_did(&self, did: &DID) -> Result<AgentIdentity, KYAError> {
        let did_str = did.to_string();
        
        let row = sqlx::query!(
            r#"
            SELECT did, method, network, identifier, name, description, 
                   owner_address, public_key, capabilities, service_endpoints,
                   created_at, updated_at
            FROM kya_agent_identities
            WHERE did = $1
            "#,
            did_str
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| KYAError::IdentityNotFound(did_str.clone()))?;

        let capabilities: Vec<Capability> = serde_json::from_value(row.capabilities)?;
        let service_endpoints: Vec<ServiceEndpoint> = serde_json::from_value(row.service_endpoints)?;

        let profile = AgentProfile {
            did: DID::new(&row.method, &row.network, &row.identifier),
            name: row.name,
            description: row.description,
            service_endpoints,
            capabilities,
            owner_address: row.owner_address,
            public_key: row.public_key,
            created_at: row.created_at,
            updated_at: row.updated_at,
        };

        Ok(AgentIdentity::from_profile(profile))
    }

    /// Update agent profile
    pub async fn update_profile(&self, profile: &AgentProfile) -> Result<(), KYAError> {
        let capabilities_json = serde_json::to_value(&profile.capabilities)?;
        let endpoints_json = serde_json::to_value(&profile.service_endpoints)?;

        sqlx::query!(
            r#"
            UPDATE kya_agent_identities
            SET name = $1, description = $2, capabilities = $3, 
                service_endpoints = $4, updated_at = $5
            WHERE did = $6
            "#,
            profile.name,
            profile.description,
            capabilities_json,
            endpoints_json,
            Utc::now(),
            profile.did.to_string()
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List all registered agents
    pub async fn list_agents(&self, limit: i64, offset: i64) -> Result<Vec<AgentIdentity>, KYAError> {
        let rows = sqlx::query!(
            r#"
            SELECT did, method, network, identifier, name, description,
                   owner_address, public_key, capabilities, service_endpoints,
                   created_at, updated_at
            FROM kya_agent_identities
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        let mut agents = Vec::new();
        for row in rows {
            let capabilities: Vec<Capability> = serde_json::from_value(row.capabilities)?;
            let service_endpoints: Vec<ServiceEndpoint> = serde_json::from_value(row.service_endpoints)?;

            let profile = AgentProfile {
                did: DID::new(&row.method, &row.network, &row.identifier),
                name: row.name,
                description: row.description,
                service_endpoints,
                capabilities,
                owner_address: row.owner_address,
                public_key: row.public_key,
                created_at: row.created_at,
                updated_at: row.updated_at,
            };

            agents.push(AgentIdentity::from_profile(profile));
        }

        Ok(agents)
    }
}
