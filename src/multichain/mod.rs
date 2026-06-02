//! Issue #532 – Multi-Chain Settlement Interoperability & Cross-EVM Bridges
//!
//! Components:
//!  - [`models`]       – data types for swaps, proofs, gateways
//!  - [`evm_client`]   – async EVM RPC client with nonce management & gas escalation
//!  - [`state_relay`]  – Merkle proof verifier and atomic settlement loop
//!  - [`metrics`]      – Prometheus observability

pub mod evm_client;
pub mod metrics;
pub mod models;
pub mod state_relay;

#[cfg(test)]
mod tests;
