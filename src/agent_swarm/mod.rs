/// Agent Swarm Intelligence — Decentralized Coordination Layer
///
/// Provides P2P peer discovery (DHT routing table), market-based task
/// decomposition & delegation, majority-voting consensus, lightweight
/// gossip state synchronisation, and x402 on-chain settlement for
/// inter-agent bounty payments.
pub mod consensus;
pub mod delegation;
pub mod discovery;
pub mod gossip;
pub mod handlers;
pub mod routes;
pub mod settlement;
pub mod types;
