use thiserror::Error;

#[derive(Error, Debug)]
pub enum KYAError {
    #[error("Identity not found: {0}")]
    IdentityNotFound(String),

    #[error("Invalid DID format: {0}")]
    InvalidDID(String),

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Attestation verification failed: {0}")]
    AttestationVerificationFailed(String),

    #[error("ZK proof verification failed")]
    ZKProofVerificationFailed,

    #[error("Unauthorized feedback submission")]
    UnauthorizedFeedback,

    #[error("Sybil attack detected")]
    SybilAttackDetected,

    #[error("Invalid reputation score")]
    InvalidReputationScore,

    #[error("Domain not supported: {0}")]
    DomainNotSupported(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    #[error("Cross-platform verification failed")]
    CrossPlatformVerificationFailed,
}

impl From<sqlx::Error> for KYAError {
    fn from(err: sqlx::Error) -> Self {
        KYAError::DatabaseError(err.to_string())
    }
}

impl From<serde_json::Error> for KYAError {
    fn from(err: serde_json::Error) -> Self {
        KYAError::SerializationError(err.to_string())
    }
}
