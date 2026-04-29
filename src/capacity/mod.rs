/// Capacity Planning & Forecasting Engine — Issue #CAPACITY-001
///
/// Modules:
///   types      — domain types, RCU model, cloud pricing, API DTOs
///   repository — all DB queries
///   forecaster — ARIMA-style OLS time-series forecasting
///   engine     — orchestration: ingest, RCU update, forecast, scenario, alerts, reports
///   handlers   — Axum HTTP handlers
///   routes     — route registration (internal + management)
///   worker     — background worker (6h cycle)
///   tests      — unit tests
pub mod engine;
pub mod forecaster;
pub mod handlers;
pub mod repository;
pub mod routes;
pub mod types;
pub mod worker;

#[cfg(test)]
mod tests;
