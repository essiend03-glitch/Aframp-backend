/// Agent Admin Dashboard — Human-in-the-Loop (HITL) Control System
///
/// Provides real-time telemetry, intervention protocols (pause/reset/circuit-breaker),
/// a Human Approval Queue for high-risk tasks, reasoning trace viewer,
/// swarm template management, and audit-ready export.
pub mod handlers;
pub mod repository;
pub mod routes;
pub mod service;
pub mod types;
