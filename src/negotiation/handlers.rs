use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::negotiation::{
    audit::{log_evidence_package, log_negotiation_event},
    engine::NegotiationEngine,
    escrow::SorobanEscrow,
    models::{
        AcceptRequest, CounterOfferRequest, InitiateNegotiationRequest, NegotiationResponse,
    },
    repository::NegotiationRepository,
    x402::X402EntranceFee,
};
use crate::audit::{models::AuditOutcome, writer::AuditWriter};

pub struct NegotiationState {
    pub repo: NegotiationRepository,
    pub fee_guard: Arc<X402EntranceFee>,
    pub escrow: Arc<SorobanEscrow>,
    pub audit: Arc<AuditWriter>,
}

pub async fn initiate(
    State(state): State<Arc<NegotiationState>>,
    Json(req): Json<InitiateNegotiationRequest>,
) -> Result<Json<NegotiationResponse>, StatusCode> {
    // 1. Verify x402 entrance fee
    state
        .fee_guard
        .verify(&req.entrance_payment_ref)
        .await
        .map_err(|_| StatusCode::PAYMENT_REQUIRED)?;

    // 2. Create session via state machine
    let session = NegotiationEngine::initiate(
        req.constraints.agent_id.clone(),
        req.responder_id,
        req.proposal,
        req.entrance_payment_ref,
    )
    .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    log_negotiation_event(&state.audit, &session, "proposed", AuditOutcome::Success, None).await;

    let id = session.id;
    state.repo.save(session).await;

    Ok(Json(NegotiationResponse {
        session_id: id,
        state: crate::negotiation::models::NegotiationState::Proposed,
        contract_id: None,
        message: "Negotiation initiated".into(),
    }))
}

pub async fn counter_offer(
    State(state): State<Arc<NegotiationState>>,
    Json(req): Json<CounterOfferRequest>,
) -> Result<Json<NegotiationResponse>, StatusCode> {
    let mut session = state
        .repo
        .get(req.session_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Constraints are re-supplied per round; in a real system they'd be stored
    // on the session or fetched from the Agentic Identity Framework.
    let constraints = crate::negotiation::models::AgentConstraints {
        agent_id: session.responder_id.clone(),
        max_price: i64::MAX,
        min_sla_score: 0,
        reputation_weight: 1.0,
    };

    NegotiationEngine::counter_offer(
        &mut session,
        session.responder_id.clone(),
        req.proposal,
        &constraints,
    )
    .map_err(|e| {
        tracing::warn!(error = %e, "counter-offer rejected");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;

    log_negotiation_event(
        &state.audit,
        &session,
        "counter_offer",
        AuditOutcome::Success,
        None,
    )
    .await;

    let current_state = session.state;
    state.repo.update(session).await;

    Ok(Json(NegotiationResponse {
        session_id: req.session_id,
        state: current_state,
        contract_id: None,
        message: "Counter-offer submitted".into(),
    }))
}

pub async fn accept(
    State(state): State<Arc<NegotiationState>>,
    Json(req): Json<AcceptRequest>,
) -> Result<Json<NegotiationResponse>, StatusCode> {
    let mut session = state
        .repo
        .get(req.session_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    NegotiationEngine::accept(&mut session).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Deploy Soroban escrow contract
    let last_round = session.rounds.last().unwrap();
    let contract_id = state
        .escrow
        .deploy(
            &session.id.to_string(),
            &session.initiator_id,
            &session.responder_id,
            last_round.proposal.base_price,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    NegotiationEngine::sign_contract(&mut session, contract_id.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    log_evidence_package(&state.audit, &session).await;

    state.repo.update(session).await;

    Ok(Json(NegotiationResponse {
        session_id: req.session_id,
        state: crate::negotiation::models::NegotiationState::ContractSigned,
        contract_id: Some(contract_id),
        message: "Terms accepted and escrow contract deployed".into(),
    }))
}
