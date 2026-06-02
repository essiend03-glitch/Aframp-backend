//! #487 Smart Order Routing & Treasury Rebalancing module.

pub mod engine;
pub mod metrics;
pub mod models;
pub mod repository;
pub mod worker;

#[cfg(test)]
mod tests;
