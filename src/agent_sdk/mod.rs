/// Open-Source AI Agent SDK for Stellar (Issue #338)
///
/// Provides a high-level, intent-based API for autonomous AI agents to manage
/// their own economic lifecycle on the Stellar network using cNGN.
///
/// # Quick Start
/// ```rust,no_run
/// use bitmesh_backend::agent_sdk::{AgentBuilder, AgentConfig};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let agent = AgentBuilder::new("my-agent")
///         .with_testnet()
///         .build()
///         .await?;
///
///     agent.pay("100", "GCEZWKCA5VLDNRLN3RPRJMRZOX3Z6G5CHCGZWM9CQJUQE3QLQHKQHQ").await?;
///     Ok(())
/// }
/// ```
pub mod agent;
pub mod error;
pub mod identity;
pub mod x402;

pub use agent::{Agent, AgentBuilder, AgentConfig, PayResult, SwapResult};
pub use error::AgentError;
pub use identity::AgentIdentity;
pub use x402::X402Client;
