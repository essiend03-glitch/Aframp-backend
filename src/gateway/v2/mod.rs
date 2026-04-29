//! Stellar Oxide Gateway v2.0
//!
//! Production-grade Rust-native gateway for the x402-stellar protocol.
//! Provides modular Auth, Rate-Limit, Payment-Verifier, and Proxy services
//! wired together via an Axum router.

pub mod auth;
pub mod health;
pub mod payment_verifier;
pub mod proxy;
pub mod rate_limit;
pub mod router;
