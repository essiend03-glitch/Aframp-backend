//! Cache-specific error types

use std::fmt;

#[derive(Debug)]
pub enum CacheError {
    ConnectionError(String),
    SerializationError(String),
    KeyError(String),
    TtlError(String),
    OperationError(String),
    LockError(String),
    ConfigurationError(String),
    InvalidatonError(String),
    PerformanceError(String),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheError::ConnectionError(msg) => write!(f, "Cache connection error: {}", msg),
            CacheError::SerializationError(msg) => write!(f, "Cache serialization error: {}", msg),
            CacheError::KeyError(msg) => write!(f, "Cache key error: {}", msg),
            CacheError::TtlError(msg) => write!(f, "Cache TTL error: {}", msg),
            CacheError::OperationError(msg) => write!(f, "Cache operation error: {}", msg),
            CacheError::LockError(msg) => write!(f, "Cache lock error: {}", msg),
            CacheError::ConfigurationError(msg) => write!(f, "Cache configuration error: {}", msg),
            CacheError::InvalidatonError(msg) => write!(f, "Cache invalidation error: {}", msg),
            CacheError::PerformanceError(msg) => write!(f, "Cache performance error: {}", msg),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<redis::RedisError> for CacheError {
    fn from(err: redis::RedisError) -> Self {
        CacheError::ConnectionError(err.to_string())
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(err: serde_json::Error) -> Self {
        CacheError::SerializationError(err.to_string())
    }
}

impl From<bb8::RunError<redis::RedisError>> for CacheError {
    fn from(err: bb8::RunError<redis::RedisError>) -> Self {
        CacheError::ConnectionError(format!("Pool error: {}", err))
    }
}

pub type CacheResult<T> = Result<T, CacheError>;
