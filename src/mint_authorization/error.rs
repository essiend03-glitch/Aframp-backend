//! Domain errors for the Mint Authorization Framework.

use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum MintAuthError {
    #[error("Reserve verification {0} not found or not approved")]
    ReserveVerificationNotFound(Uuid),

    #[error("Reserve verification is too old (max recency: {max_hours}h, actual: {actual_hours:.1}h)")]
    ReserveVerificationStale { max_hours: i64, actual_hours: f64 },

    #[error("Requested mint amount {requested} exceeds available reserve balance {available}")]
    ExceedsReserveBalance { requested: String, available: String },

    #[error("Authorization request {0} not found")]
    NotFound(Uuid),

    #[error("Authorization request {0} is in terminal state: {1}")]
    TerminalState(Uuid, String),

    #[error("Authorization request {0} has expired")]
    Expired(Uuid),

    #[error("Signer {0} is not an authorized signer")]
    UnauthorizedSigner(String),

    #[error("Invalid signature from signer {0}: {1}")]
    InvalidSignature(String, String),

    #[error("Signature is over a different transaction hash (substitution attack prevented)")]
    TransactionHashMismatch,

    #[error("Signer {0} has already signed authorization {1}")]
    DuplicateSignature(String, Uuid),

    #[error("Authorization {0} is not in threshold_met state for submission")]
    NotReadyForSubmission(Uuid),

    #[error("Stellar submission failed: {0}")]
    StellarSubmission(String),

    #[error("XDR build error: {0}")]
    XdrBuild(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

impl From<sqlx::Error> for MintAuthError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e.to_string())
    }
}
