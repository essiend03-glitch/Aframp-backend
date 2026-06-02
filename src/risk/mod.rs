//! Automated Liquidity Risk Management & Circuit Breaker Engine — Issue #494.

pub mod circuit_breaker;
pub mod handlers;
pub mod metrics;
pub mod models;
pub mod repository;
pub mod volatility;

pub use circuit_breaker::CircuitBreakerEngine;
pub use handlers::{risk_routes, RiskState};
pub use models::{CircuitBreakerEvent, IsolationScope, RiskCorridorProfile};
pub use repository::RiskRepository;
pub use volatility::VolatilityScanner;
