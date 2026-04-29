//! Merchant Multi-Sig & Treasury Controls — Issue #336
//!
//! Enforces M-of-N signing requirements for high-stakes merchant actions:
//! payouts, API key updates, tax config changes. Includes emergency freeze.

pub mod handlers;
pub mod models;
pub mod routes;
pub mod service;

pub use routes::merchant_multisig_routes;
pub use service::MerchantMultisigService;

#[cfg(test)]
mod tests;
