use thiserror::Error;

/// Errors produced by the AI Agent SDK.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Identity error: {0}")]
    Identity(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: String, available: String },

    #[error("x402 payment required: {0}")]
    X402PaymentRequired(String),

    #[error("Swap failed: {0}")]
    SwapFailed(String),

    #[error("Max retries exceeded after {attempts} attempts: {last_error}")]
    MaxRetriesExceeded { attempts: u32, last_error: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

pub type AgentResult<T> = Result<T, AgentError>;
