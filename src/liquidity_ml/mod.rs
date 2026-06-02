//! Issue #531 – Real-Time Predictive Liquidity Modeling & ML Core
//!
//! Components:
//!  - [`feature_pipeline`] – async Tokio actor ingesting live payment frames
//!  - [`inference`]        – embedded ONNX/static inference engine with fallback
//!  - [`rebalance`]        – preemptive rebalancing trigger loop
//!  - [`metrics`]          – Prometheus observability

pub mod feature_pipeline;
pub mod inference;
pub mod metrics;
pub mod rebalance;

#[cfg(test)]
mod tests;
