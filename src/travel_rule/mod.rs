/// Travel Rule Compliance (Issue #393)
///
/// Implements FATF Recommendation 16 for VASP-to-VASP PII exchange.
/// Supports TRISA, TRUST, OpenVASP, and IVMS101 direct protocols.
///
/// Key behaviours:
/// - Outbound transfers above threshold enter "pending-travel-rule" state
/// - Protocol router selects best supported protocol per destination VASP
/// - Encrypted off-chain PII exchange (IVMS101 schema)
/// - Inbound transfers verified against internal KYC profile before crediting
/// - Unknown/unhosted wallets routed to manual compliance review
/// - All exchanges logged for audit trail
pub mod models;
pub mod service;

pub use models::*;
pub use service::TravelRuleService;
