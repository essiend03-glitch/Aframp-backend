use crate::kya::{error::KYAError, models::*};
use chrono::{DateTime, Utc};
use ed25519_dalek::{PublicKey, Signature, Verifier};
use sqlx::PgPool;
use uuid::Uuid;

/// Cryptographically signed attestation of agent performance
pub struct Attestation {
    pool: PgPool,
}

impl Attestation {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create and store a new attestation
    pub async fn create(
        &self,
        agent_did: &DID,
        issuer_did: &DID,
        domain: &ReputationDomain,
        claim: String,
        evidence_uri: Option<String>,
        signature: String,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<AttestationRecord, KYAError> {
        let attestation = AttestationRecord {
            id: Uuid::new_v4(),
            agent_did: agent_did.clone(),
            issuer_did: issuer_did.clone(),
            domain: domain.clone(),
            claim,
            evidence_uri,
            signature,
            issued_at: Utc::now(),
            expires_at,
        };

        sqlx::query!(
            r#"
            INSERT INTO kya_attestations
            (id, agent_did, issuer_did, domain, claim, evidence_uri, 
             signature, issued_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            attestation.id,
            attestation.agent_did.to_string(),
            attestation.issuer_did.to_string(),
            attestation.domain.as_str(),
            attestation.claim,
            attestation.evidence_uri,
            attestation.signature,
            attestation.issued_at,
            attestation.expires_at
        )
        .execute(&self.pool)
        .await?;

        Ok(attestation)
    }

    /// Get all attestations for an agent
    pub async fn get_by_agent(&self, agent_did: &DID) -> Result<Vec<AttestationRecord>, KYAError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, agent_did, issuer_did, domain, claim, evidence_uri,
                   signature, issued_at, expires_at
            FROM kya_attestations
            WHERE agent_did = $1
            AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY issued_at DESC
            "#,
            agent_did.to_string()
        )
        .fetch_all(&self.pool)
        .await?;

        let mut attestations = Vec::new();
        for row in rows {
            let domain = match row.domain.as_str() {
                "code_audit" => ReputationDomain::CodeAudit,
                "financial_analysis" => ReputationDomain::FinancialAnalysis,
                "content_creation" => ReputationDomain::ContentCreation,
                "data_processing" => ReputationDomain::DataProcessing,
                "smart_contract_execution" => ReputationDomain::SmartContractExecution,
                "payment_processing" => ReputationDomain::PaymentProcessing,
                custom => ReputationDomain::Custom(custom.to_string()),
            };

            attestations.push(AttestationRecord {
                id: row.id,
                agent_did: DID::from_string(&row.agent_did)?,
                issuer_did: DID::from_string(&row.issuer_did)?,
                domain,
                claim: row.claim,
                evidence_uri: row.evidence_uri,
                signature: row.signature,
                issued_at: row.issued_at,
                expires_at: row.expires_at,
            });
        }

        Ok(attestations)
    }

    /// Get attestations by domain
    pub async fn get_by_domain(
        &self,
        agent_did: &DID,
        domain: &ReputationDomain,
    ) -> Result<Vec<AttestationRecord>, KYAError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, agent_did, issuer_did, domain, claim, evidence_uri,
                   signature, issued_at, expires_at
            FROM kya_attestations
            WHERE agent_did = $1 AND domain = $2
            AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY issued_at DESC
            "#,
            agent_did.to_string(),
            domain.as_str()
        )
        .fetch_all(&self.pool)
        .await?;

        let mut attestations = Vec::new();
        for row in rows {
            attestations.push(AttestationRecord {
                id: row.id,
                agent_did: DID::from_string(&row.agent_did)?,
                issuer_did: DID::from_string(&row.issuer_did)?,
                domain: domain.clone(),
                claim: row.claim,
                evidence_uri: row.evidence_uri,
                signature: row.signature,
                issued_at: row.issued_at,
                expires_at: row.expires_at,
            });
        }

        Ok(attestations)
    }
}

/// Verifies attestation signatures
pub struct AttestationVerifier {
    pool: PgPool,
}

impl AttestationVerifier {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Verify an attestation's cryptographic signature
    pub async fn verify(&self, attestation: &AttestationRecord) -> Result<bool, KYAError> {
        // Fetch issuer's public key
        let issuer_row = sqlx::query!(
            r#"
            SELECT public_key
            FROM kya_agent_identities
            WHERE did = $1
            "#,
            attestation.issuer_did.to_string()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| KYAError::IdentityNotFound(attestation.issuer_did.to_string()))?;

        let public_key_bytes = hex::decode(&issuer_row.public_key)
            .map_err(|e| KYAError::CryptoError(e.to_string()))?;

        let public_key = PublicKey::from_bytes(&public_key_bytes)
            .map_err(|e| KYAError::CryptoError(e.to_string()))?;

        // Construct message to verify
        let message = format!(
            "{}:{}:{}:{}",
            attestation.agent_did.to_string(),
            attestation.domain.as_str(),
            attestation.claim,
            attestation.issued_at.to_rfc3339()
        );

        let signature_bytes = hex::decode(&attestation.signature)
            .map_err(|e| KYAError::CryptoError(e.to_string()))?;

        let signature = Signature::from_bytes(&signature_bytes)
            .map_err(|e| KYAError::CryptoError(e.to_string()))?;

        Ok(public_key.verify(message.as_bytes(), &signature).is_ok())
    }

    /// Verify all attestations for an agent
    pub async fn verify_all(&self, agent_did: &DID) -> Result<Vec<(AttestationRecord, bool)>, KYAError> {
        let attestation_manager = Attestation::new(self.pool.clone());
        let attestations = attestation_manager.get_by_agent(agent_did).await?;

        let mut results = Vec::new();
        for attestation in attestations {
            let is_valid = self.verify(&attestation).await?;
            results.push((attestation, is_valid));
        }

        Ok(results)
    }
}
