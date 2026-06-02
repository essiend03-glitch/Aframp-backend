//! Stellar Ecosystem Partner Integration (Issue #470).
//! SEP-24/SEP-31 anchor client, DEX pathfinding, transaction builder, slippage protection.

pub mod dex_pathfinding;
pub mod handlers;
pub mod metrics;
pub mod models;
pub mod repository;
pub mod routes;
pub mod sep_client;
pub mod service;
pub mod transaction_builder;

#[cfg(feature = "database")]
pub use handlers::*;
#[cfg(feature = "database")]
pub use models::*;
#[cfg(feature = "database")]
pub use routes::*;
#[cfg(feature = "database")]
pub use service::{EcosystemConfig, EcosystemService};
