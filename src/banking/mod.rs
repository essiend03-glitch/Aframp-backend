//! Banking Partner Integration & Account Linkage (Issue #407, #407 Extended)
//!
//! Provides:
//! - Secure bank account linkage with BVN/NIN identity verification
//! - Tokenized storage (no plaintext credentials)
//! - Direct debit/credit mandate management
//! - Idempotent fund transfers via Paystack/Flutterwave
//! - Daily reconciliation engine (Aframp ledger vs bank EOD statement)
//! - Inbound webhook processing with idempotent event store
//! - Bank integrations for corporate partners
//! - Virtual account orchestration for dedicated collection accounts
//! - Fiat-to-cNGN settlement execution

pub mod admin;
pub mod handlers;
pub mod integrations;
pub mod metrics;
pub mod models;
pub mod reconciliation;
pub mod repository;
pub mod routes;
pub mod service;
pub mod verification;
pub mod virtual_accounts;
pub mod webhook;
pub mod webhook_ingestion;
pub mod fiat_settlement;

pub use admin::BankingAdminService;
pub use integrations::{
    BankIntegration, BankPartnerStatusResponse, FiatSettlement, FiatSettlementResponse,
    VirtualAccount, VirtualAccountState,
};
pub use metrics::BankingMetricsService;
pub use models::{
    BankMandate, BankReconciliationRun, BankTransferLog, BankWebhookEvent, LinkedBankAccount,
};
pub use reconciliation::ReconciliationEngine;
pub use repository::BankingRepository;
pub use routes::{banking_routes, banking_webhook_routes};
pub use service::BankingService;
pub use verification::{BankVerificationService, VerificationError};
pub use virtual_accounts::{VirtualAccountOrchestrator, VirtualAccountError};
pub use webhook::BankWebhookProcessor;
pub use webhook_ingestion::{WebhookIngestionController, WebhookError};
pub use fiat_settlement::{FiatSettlementExecutor, SettlementError};
