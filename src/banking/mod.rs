//! Banking Partner Integration & Account Linkage (Issue #407)
//!
//! Provides:
//! - Secure bank account linkage with BVN/NIN identity verification
//! - Tokenized storage (no plaintext credentials)
//! - Direct debit/credit mandate management
//! - Idempotent fund transfers via Paystack/Flutterwave
//! - Daily reconciliation engine (Aframp ledger vs bank EOD statement)
//! - Inbound webhook processing with idempotent event store

pub mod handlers;
pub mod models;
pub mod reconciliation;
pub mod repository;
pub mod routes;
pub mod service;
pub mod webhook;

pub use models::{
    BankMandate, BankReconciliationRun, BankTransferLog, BankWebhookEvent, LinkedBankAccount,
};
pub use reconciliation::ReconciliationEngine;
pub use repository::BankingRepository;
pub use routes::{banking_routes, banking_webhook_routes};
pub use service::BankingService;
pub use webhook::BankWebhookProcessor;
