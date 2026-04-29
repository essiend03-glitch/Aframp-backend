/// In-House CFO — Autonomous Agent Treasury Management
///
/// Provides budget policy enforcement, expenditure visibility, graceful
/// degradation, automated wallet refills, and a burn-rate kill-switch for
/// AI agents operating with revolving budgets.
pub mod engine;
pub mod handlers;
pub mod ledger;
pub mod routes;
pub mod types;
pub mod watchdog;
