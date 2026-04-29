use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Peer discovery ────────────────────────────────────────────────────────────

/// Trust / capability tier of a peer in the swarm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "peer_tier", rename_all = "snake_case")]
pub enum PeerTier {
    /// Newly joined — limited delegation rights.
    Provisional,
    /// Verified track record — full delegation rights.
    Trusted,
    /// Banned due to repeated failures or malicious behaviour.
    Revoked,
}

/// A peer node in the swarm DHT routing table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SwarmPeer {
    pub id: Uuid,
    /// Stable peer identifier (SHA-256 of public key, hex).
    pub peer_id: String,
    /// Agent registry ID this peer maps to.
    pub agent_id: Uuid,
    /// Reachable endpoint (e.g. "https://agent-42.internal:8443").
    pub endpoint: String,
    pub tier: PeerTier,
    /// Reputation score 0–100 (updated after each task settlement).
    pub reputation: i32,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

// ── Task decomposition & delegation ──────────────────────────────────────────

/// Status of a swarm task request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "swarm_task_status", rename_all = "snake_case")]
pub enum SwarmTaskStatus {
    /// Broadcast to the swarm, awaiting bids.
    Open,
    /// All micro-tasks assigned; execution in progress.
    InProgress,
    /// Consensus reached; result committed.
    Completed,
    /// Consensus failed or timed out.
    Failed,
}

/// A complex task broadcast by a Manager Agent with a cNGN bounty.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SwarmTaskRequest {
    pub id: Uuid,
    pub manager_agent_id: Uuid,
    pub description: String,
    /// Total cNGN bounty split across micro-tasks.
    pub total_bounty_cngn: String,
    pub status: SwarmTaskStatus,
    /// Minimum votes required for consensus (majority = ceil(assigned/2)+1).
    pub required_votes: i32,
    pub result_payload: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// A single micro-task delegated to one subordinate agent.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MicroTask {
    pub id: Uuid,
    pub swarm_task_id: Uuid,
    pub assignee_agent_id: Uuid,
    pub description: String,
    /// cNGN bounty for this specific micro-task.
    pub bounty_cngn: String,
    /// "pending" | "running" | "submitted" | "accepted" | "rejected"
    pub status: String,
    pub result_payload: Option<serde_json::Value>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── Consensus / voting ────────────────────────────────────────────────────────

/// A single vote cast by an agent on a swarm task result.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConsensusVote {
    pub id: Uuid,
    pub swarm_task_id: Uuid,
    pub voter_agent_id: Uuid,
    /// SHA-256 of the result payload the voter agrees with.
    pub result_hash: String,
    pub cast_at: DateTime<Utc>,
}

/// Outcome of a consensus round.
#[derive(Debug, Serialize)]
pub struct ConsensusOutcome {
    pub swarm_task_id: Uuid,
    pub reached: bool,
    pub winning_hash: Option<String>,
    pub vote_count: i64,
    pub required_votes: i32,
}

// ── Gossip state ──────────────────────────────────────────────────────────────

/// A single gossip state entry — shared market intelligence across the swarm.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GossipEntry {
    pub id: Uuid,
    /// Logical key (e.g. "market.xlm_usd", "swarm.available_rewards").
    pub state_key: String,
    pub value: serde_json::Value,
    /// Lamport-style version counter for conflict resolution.
    pub version: i64,
    pub origin_peer_id: String,
    pub updated_at: DateTime<Utc>,
}

// ── On-chain settlement ───────────────────────────────────────────────────────

/// x402 payment settlement record for a completed micro-task.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SwarmSettlement {
    pub id: Uuid,
    pub swarm_task_id: Uuid,
    pub micro_task_id: Uuid,
    pub payer_agent_id: Uuid,
    pub payee_agent_id: Uuid,
    pub amount_cngn: String,
    pub stellar_tx_hash: Option<String>,
    /// "pending" | "submitted" | "confirmed" | "failed"
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

// ── Request / Response ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterPeerRequest {
    pub peer_id: String,
    pub agent_id: Uuid,
    pub endpoint: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSwarmTaskRequest {
    pub manager_agent_id: Uuid,
    pub description: String,
    pub total_bounty_cngn: String,
    /// Sub-task descriptions with individual bounties.
    pub micro_tasks: Vec<MicroTaskSpec>,
    pub required_votes: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct MicroTaskSpec {
    pub description: String,
    pub bounty_cngn: String,
    pub assignee_agent_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct SubmitMicroTaskRequest {
    pub agent_id: Uuid,
    pub result_payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct CastVoteRequest {
    pub voter_agent_id: Uuid,
    /// SHA-256 of the result payload being endorsed.
    pub result_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct GossipPushRequest {
    pub state_key: String,
    pub value: serde_json::Value,
    pub version: i64,
    pub origin_peer_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PeerListQuery {
    pub tier: Option<PeerTier>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl PeerListQuery {
    pub fn page(&self) -> i64 { self.page.unwrap_or(1).max(1) }
    pub fn page_size(&self) -> i64 { self.page_size.unwrap_or(50).clamp(1, 200) }
    pub fn offset(&self) -> i64 { (self.page() - 1) * self.page_size() }
}
