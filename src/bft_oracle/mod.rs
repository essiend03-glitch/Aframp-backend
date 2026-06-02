//! Issue #533 – Decentralized Oracles, BFT Price Feeds & MEV Protection Core
//!
//! Components:
//!  - [`medianizer`]  – BFT weighted-median with Winsorized outlier filter (4σ)
//!  - [`mev_shield`]  – Flash-loan interceptor and tx-delay randomizer
//!  - [`metrics`]     – Prometheus observability

pub mod medianizer;
pub mod metrics;
pub mod mev_shield;
pub mod models;

#[cfg(test)]
mod tests;
