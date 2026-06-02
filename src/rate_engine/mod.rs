//! Issue #530 – Multi-Tenant Resource Isolation, Rate Limiting & Fair-Share Scheduling Engine
//!
//! Components:
//!  - [`TokenBucket`]   – per-tenant hierarchical token bucket (HTB) with Redis atomic Lua scripts
//!  - [`DrqScheduler`]  – deficit round-robin fair-share scheduler backed by crossbeam channels
//!  - [`RateLimitMiddleware`] – Axum layer returning HTTP 429 with Retry-After on exhaustion

pub mod models;
pub mod scheduler;
pub mod token_bucket;
pub mod middleware;
pub mod metrics;

#[cfg(test)]
mod tests;
