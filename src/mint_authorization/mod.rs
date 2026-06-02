//! Mint Authorization Framework (#213)
//!
//! Governs cNGN issuance via M-of-N multi-signature approval before Stellar submission.
//!
//! # Flow
//! ```text
//! Admin
//!   │
//!   ▼
//! [POST /authorizations]  ──► validate reserve ──► build unsigned XDR ──► persist
//!   │
//!   ▼
//! [notify signers]
//!   │
//!   ▼
//! [POST /authorizations/:id/sign]  ──► verify Ed25519 sig ──► persist ──► check threshold
//!   │
//!   ▼ (threshold met)
//! [aggregate signatures into XDR]  ──► submit to Stellar Horizon (retry w/ backoff)
//!   │
//!   ▼
//! [monitor confirmation]  ──► confirmed / failed
//! ```

pub mod error;
pub mod handlers;
pub mod metrics;
pub mod models;
pub mod repository;
pub mod routes;
pub mod service;
pub mod worker;

pub use error::MintAuthError;
pub use models::{MintAuthRequest, MintAuthSignature, MintAuthStatus};
pub use service::MintAuthService;
