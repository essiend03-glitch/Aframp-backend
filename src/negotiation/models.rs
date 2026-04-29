use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lifecycle states of a negotiation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "negotiation_state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NegotiationState {
    Proposed,
    CounterOffer,
    Accepted,
    ContractSigned,
    Failed,
}

/// A single proposal / counter-offer exchanged between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationProposal {
    pub service_id: String,
    /// Amount in the platform's base unit (e.g. stroops or minor currency units).
    pub base_price: i64,
    /// Free-form SLA terms (e.g. "99.9% uptime, 200ms p99").
    pub sla_terms: String,
    /// Wall-clock expiry of this specific proposal.
    pub expiry: DateTime<Utc>,
}

/// Full negotiation session record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationSession {
    pub id: Uuid,
    pub initiator_id: String,
    pub responder_id: String,
    pub state: NegotiationState,
    /// Ordered history of proposals (index 0 = original offer).
    pub rounds: Vec<NegotiationRound>,
    /// x402 payment reference that unlocked this session.
    pub entrance_payment_ref: String,
    /// Soroban contract ID once state == ContractSigned.
    pub contract_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationRound {
    pub round: u32,
    pub proposer_id: String,
    pub proposal: NegotiationProposal,
    pub submitted_at: DateTime<Utc>,
}

/// Walk-away constraints an agent registers before entering a negotiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConstraints {
    pub agent_id: String,
    pub max_price: i64,
    pub min_sla_score: u8, // 0-100
    pub reputation_weight: f64, // multiplier applied to price based on counterparty score
}

// ── HTTP request / response shapes ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct InitiateNegotiationRequest {
    pub responder_id: String,
    pub proposal: NegotiationProposal,
    pub constraints: AgentConstraints,
    /// x402 payment proof (e.g. Stellar transaction hash).
    pub entrance_payment_ref: String,
}

#[derive(Debug, Deserialize)]
pub struct CounterOfferRequest {
    pub session_id: Uuid,
    pub proposal: NegotiationProposal,
}

#[derive(Debug, Deserialize)]
pub struct AcceptRequest {
    pub session_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct NegotiationResponse {
    pub session_id: Uuid,
    pub state: NegotiationState,
    pub contract_id: Option<String>,
    pub message: String,
}
