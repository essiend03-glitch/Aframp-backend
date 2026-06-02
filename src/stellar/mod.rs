/// High-throughput Stellar transaction submission pipeline
/// 
/// This module implements a resilient, parallelized transaction submission engine that:
/// - Maintains a pool of channel accounts for sequence number distribution
/// - Uses lock-free atomic counters for sequence coordination
/// - Dynamically adjusts fees based on Horizon surge pricing
/// - Implements retry logic with exponential backoff and channel rotation
/// - Tracks all submissions in an immutable audit ledger

pub mod channel_pool;
pub mod fee_engine;
pub mod sequence_coordinator;
pub mod submission;
pub mod error;
pub mod models;
pub mod horizon;
pub mod retry_state_machine;
pub mod metrics;
pub mod admin;

pub use channel_pool::ChannelPool;
pub use fee_engine::DynamicFeeEngine;
pub use sequence_coordinator::SequenceCoordinator;
pub use submission::StellarSubmissionEngine;
pub use error::{SubmissionError, SubmissionResult};
pub use models::*;
pub use retry_state_machine::RetryStateMachine;

