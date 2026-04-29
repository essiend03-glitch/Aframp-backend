use crate::audit::{
    models::{AuditActorType, AuditEventCategory, AuditOutcome, PendingAuditEntry},
    writer::AuditWriter,
};
use crate::negotiation::models::{NegotiationSession, NegotiationState};

/// Emit an audit entry for every negotiation state transition.
pub async fn log_negotiation_event(
    writer: &AuditWriter,
    session: &NegotiationSession,
    event_type: &str,
    outcome: AuditOutcome,
    failure_reason: Option<String>,
) {
    let entry = PendingAuditEntry {
        event_type: format!("negotiation.{}", event_type),
        event_category: AuditEventCategory::FinancialTransaction,
        actor_type: AuditActorType::Microservice,
        actor_id: Some(session.initiator_id.clone()),
        actor_ip: None,
        actor_consumer_type: Some("autonomous_agent".into()),
        session_id: Some(session.id.to_string()),
        target_resource_type: Some("negotiation_session".into()),
        target_resource_id: Some(session.id.to_string()),
        request_method: "INTERNAL".into(),
        request_path: "/negotiation/state-machine".into(),
        request_body_hash: None,
        response_status: if outcome == AuditOutcome::Success { 200 } else { 422 },
        response_latency_ms: 0,
        outcome,
        failure_reason,
        environment: std::env::var("APP_ENV").unwrap_or_else(|_| "development".into()),
    };
    writer.write(entry).await;
}

/// Emit the full Negotiation Evidence Package when a session reaches a terminal state.
pub async fn log_evidence_package(writer: &AuditWriter, session: &NegotiationSession) {
    let (event, outcome) = match session.state {
        NegotiationState::ContractSigned => ("contract_signed", AuditOutcome::Success),
        NegotiationState::Failed => ("negotiation_failed", AuditOutcome::Failure),
        _ => return,
    };
    log_negotiation_event(writer, session, event, outcome, None).await;
}
