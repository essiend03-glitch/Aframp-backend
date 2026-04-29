use crate::negotiation::models::{
    AgentConstraints, NegotiationProposal, NegotiationRound, NegotiationSession, NegotiationState,
};
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum NegotiationError {
    #[error("invalid state transition: {0:?} -> {1:?}")]
    InvalidTransition(NegotiationState, NegotiationState),
    #[error("proposal expired")]
    ProposalExpired,
    #[error("walk-away threshold exceeded: price {0} > max {1}")]
    WalkAwayThreshold(i64, i64),
    #[error("session not found")]
    SessionNotFound,
    #[error("x402 entrance fee not verified")]
    EntranceFeeNotVerified,
}

pub struct NegotiationEngine;

impl NegotiationEngine {
    /// Create a new session after the x402 entrance fee is verified.
    pub fn initiate(
        initiator_id: String,
        responder_id: String,
        proposal: NegotiationProposal,
        entrance_payment_ref: String,
    ) -> Result<NegotiationSession, NegotiationError> {
        if proposal.expiry <= Utc::now() {
            return Err(NegotiationError::ProposalExpired);
        }
        let now = Utc::now();
        Ok(NegotiationSession {
            id: Uuid::new_v4(),
            initiator_id: initiator_id.clone(),
            responder_id,
            state: NegotiationState::Proposed,
            rounds: vec![NegotiationRound {
                round: 1,
                proposer_id: initiator_id,
                proposal,
                submitted_at: now,
            }],
            entrance_payment_ref,
            contract_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Apply a counter-offer, checking walk-away constraints.
    pub fn counter_offer(
        session: &mut NegotiationSession,
        proposer_id: String,
        proposal: NegotiationProposal,
        constraints: &AgentConstraints,
    ) -> Result<(), NegotiationError> {
        if !matches!(
            session.state,
            NegotiationState::Proposed | NegotiationState::CounterOffer
        ) {
            return Err(NegotiationError::InvalidTransition(
                session.state,
                NegotiationState::CounterOffer,
            ));
        }
        if proposal.expiry <= Utc::now() {
            return Err(NegotiationError::ProposalExpired);
        }

        // Apply reputation-adjusted walk-away check
        let effective_max = (constraints.max_price as f64 * constraints.reputation_weight) as i64;
        if proposal.base_price > effective_max {
            return Err(NegotiationError::WalkAwayThreshold(
                proposal.base_price,
                effective_max,
            ));
        }

        let round = session.rounds.len() as u32 + 1;
        session.rounds.push(NegotiationRound {
            round,
            proposer_id,
            proposal,
            submitted_at: Utc::now(),
        });
        session.state = NegotiationState::CounterOffer;
        session.updated_at = Utc::now();
        Ok(())
    }

    /// Accept the latest proposal; transitions to Accepted.
    pub fn accept(session: &mut NegotiationSession) -> Result<(), NegotiationError> {
        if !matches!(
            session.state,
            NegotiationState::Proposed | NegotiationState::CounterOffer
        ) {
            return Err(NegotiationError::InvalidTransition(
                session.state,
                NegotiationState::Accepted,
            ));
        }
        session.state = NegotiationState::Accepted;
        session.updated_at = Utc::now();
        Ok(())
    }

    /// Bind accepted terms to a Soroban contract ID.
    pub fn sign_contract(
        session: &mut NegotiationSession,
        contract_id: String,
    ) -> Result<(), NegotiationError> {
        if session.state != NegotiationState::Accepted {
            return Err(NegotiationError::InvalidTransition(
                session.state,
                NegotiationState::ContractSigned,
            ));
        }
        session.contract_id = Some(contract_id);
        session.state = NegotiationState::ContractSigned;
        session.updated_at = Utc::now();
        Ok(())
    }

    /// Mark a session as failed (no agreement) so it can be garbage-collected.
    pub fn fail(session: &mut NegotiationSession) {
        session.state = NegotiationState::Failed;
        session.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn proposal(price: i64) -> NegotiationProposal {
        NegotiationProposal {
            service_id: "svc-1".into(),
            base_price: price,
            sla_terms: "99.9% uptime".into(),
            expiry: Utc::now() + Duration::hours(1),
        }
    }

    fn constraints(max: i64) -> AgentConstraints {
        AgentConstraints {
            agent_id: "agent-b".into(),
            max_price: max,
            min_sla_score: 80,
            reputation_weight: 1.0,
        }
    }

    #[test]
    fn full_happy_path() {
        let mut session = NegotiationEngine::initiate(
            "agent-a".into(),
            "agent-b".into(),
            proposal(100),
            "tx-hash-001".into(),
        )
        .unwrap();

        assert_eq!(session.state, NegotiationState::Proposed);

        NegotiationEngine::counter_offer(
            &mut session,
            "agent-b".into(),
            proposal(90),
            &constraints(200),
        )
        .unwrap();
        assert_eq!(session.state, NegotiationState::CounterOffer);

        NegotiationEngine::accept(&mut session).unwrap();
        assert_eq!(session.state, NegotiationState::Accepted);

        NegotiationEngine::sign_contract(&mut session, "CXXX...".into()).unwrap();
        assert_eq!(session.state, NegotiationState::ContractSigned);
        assert_eq!(session.rounds.len(), 2);
    }

    #[test]
    fn walk_away_threshold_enforced() {
        let mut session = NegotiationEngine::initiate(
            "agent-a".into(),
            "agent-b".into(),
            proposal(100),
            "tx-hash-002".into(),
        )
        .unwrap();

        let err = NegotiationEngine::counter_offer(
            &mut session,
            "agent-b".into(),
            proposal(500),
            &constraints(200),
        )
        .unwrap_err();

        assert!(matches!(err, NegotiationError::WalkAwayThreshold(500, 200)));
    }

    #[test]
    fn invalid_transition_from_signed() {
        let mut session = NegotiationEngine::initiate(
            "a".into(),
            "b".into(),
            proposal(50),
            "tx-003".into(),
        )
        .unwrap();
        NegotiationEngine::accept(&mut session).unwrap();
        NegotiationEngine::sign_contract(&mut session, "C1".into()).unwrap();

        let err = NegotiationEngine::accept(&mut session).unwrap_err();
        assert!(matches!(err, NegotiationError::InvalidTransition(_, _)));
    }
}
