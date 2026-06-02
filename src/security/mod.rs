//! Security Module - Anomaly Detection & Circuit Breaker
//!
//! This module provides comprehensive security monitoring and automated response
//! for the cNGN stablecoin system.

pub mod alerts;
pub mod anomaly_detection;
pub mod halt_queue;
pub mod merkle;

#[cfg(test)]
pub mod tests;

pub use alerts::{AlertChannel, AlertConfig, AlertMessage, AlertService, AlertSeverity};
pub use anomaly_detection::{
    ensure_system_status_table, AnomalyDetectionConfig, AnomalyDetectionService,
    CircuitBreakerMiddleware, CircuitBreakerState, OnChainMint, SystemStatus,
};
pub use halt_queue::{
    HaltStatistics, HaltedTransactionRepository, HaltedTransactionStatus, SystemHaltQueueManager,
};
pub use merkle::{MerklePathNode, MerkleProof, MerkleTree, TenantBalance};
