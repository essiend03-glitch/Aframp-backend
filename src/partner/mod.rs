pub mod compliance;
pub mod monitoring;
pub mod api;
//! Partner Integration Framework — unified "Partner Hub" for banking partners,
//! fintechs, and liquidity providers.
//!
//! # Architecture
//! - `models`     — domain types (Partner, PartnerCredential, deprecation notices)
//! - `error`      — typed error enum with Axum IntoResponse
//! - `repository` — all DB queries (partners, credentials, rate counters, deprecations)
//! - `service`    — registration, credential provisioning, validation engine
//! - `middleware` — per-partner rate limiting, mTLS/OAuth2/API-key auth,
//!                  Partner_ID + Correlation_ID injection
//! - `handlers`   — Axum handler functions
//! - `routes`     — Router builder (public + authenticated sub-routers)

pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repository;
pub mod routes;
pub mod service;
pub mod worker;

pub use routes::partner_routes;
pub use worker::DeprecationNotificationWorker;
