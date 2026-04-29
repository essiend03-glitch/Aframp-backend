// KYA (Know Your Agent) - Decentralized Agent Identity & Reputation System
//
// This module implements a sovereign identity and reputation framework for autonomous agents,
// enabling trustless collaboration through:
// - DID-based identity registry
// - On-chain reputation & attestations
// - Zero-knowledge competence proofs
// - Cross-platform reputation portability
// - Sybil-resistant feedback mechanisms

pub mod identity;
pub mod reputation;
pub mod attestation;
pub mod zkp;
pub mod scoring;
pub mod registry;
pub mod models;
pub mod error;
pub mod routes;

pub use identity::{AgentIdentity, IdentityRegistry};
pub use reputation::{ReputationManager, FeedbackAuthorization};
pub use attestation::{Attestation, AttestationVerifier};
pub use zkp::{CompetenceProof, ZKProofVerifier};
pub use scoring::{DomainScore, ModularScoring};
pub use registry::KYARegistry;
pub use models::*;
pub use error::KYAError;
pub use routes::kya_routes;
