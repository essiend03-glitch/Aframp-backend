//! Central Bank Digital Currency (CBDC) Interoperability & Sandbox Bridge (Issue #499)
//!
//! This module provides:
//! - Enterprise DLT gateway client for Hyperledger Besu, Corda, and Quorum networks
//! - HSM signing client for PKCS#11-based institutional key operations
//! - Two-Phase Commit (2PC) Lock Manager backed by Redis
//! - Cross-rail settlement worker for atomic CBDC ↔ Stellar asset swaps
//! - Transaction reversal engine with safe rollback semantics
//! - AML/compliance payload validation pipeline
//! - Prometheus metrics and structured tracing for government-tier telemetry

pub mod gateway;
pub mod handlers;
pub mod hsm;
pub mod metrics;
pub mod models;
pub mod repository;
pub mod reversal;
pub mod routes;
pub mod settlement;
pub mod two_pc;
pub mod validator;

#[cfg(test)]
pub mod tests;

pub use gateway::{DltGatewayClient, DltGatewayConfig, DltSystem, GatewayConnectionStatus};
pub use handlers::CbdcHandlerState;
pub use hsm::{HsmClient, HsmClientConfig, HsmSignature, HsmSigningAlgorithm};
pub use metrics::CbdcMetrics;
pub use models::*;
pub use repository::CbdcRepository;
pub use reversal::ReversalEngine;
pub use routes::{cbdc_admin_routes, cbdc_api_routes, CbdcApiState};
pub use settlement::SettlementWorker;
pub use two_pc::{TwoPhaseCommitManager, TwoPhaseLockState};
pub use validator::SwapValidator;
