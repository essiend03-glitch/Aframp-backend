/// Event-Driven Architecture (Issue #399)
///
/// Decouples platform services via an asynchronous event bus backed by PostgreSQL
/// for durability. Core services publish events; downstream consumers (notifications,
/// analytics, reporting) subscribe and process at their own pace.
///
/// Key guarantees:
/// - At-least-once delivery via DB persistence + ACK pattern
/// - Dead-letter queue after configurable retry exhaustion
/// - Idempotent consumers via processed_events deduplication table
/// - Full event replay for recovery scenarios
pub mod models;
pub mod bus;

pub use models::*;
pub use bus::EventBus;
