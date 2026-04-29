use crate::agent_swarm::{
    consensus::ConsensusEngine,
    delegation::DelegationEngine,
    discovery::PeerDiscovery,
    gossip::GossipStore,
    settlement::SettlementEngine,
    types::{
        CastVoteRequest, CreateSwarmTaskRequest, GossipPushRequest, PeerListQuery,
        RegisterPeerRequest, SubmitMicroTaskRequest,
    },
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SwarmState {
    pub discovery: Arc<PeerDiscovery>,
    pub delegation: Arc<DelegationEngine>,
    pub consensus: Arc<ConsensusEngine>,
    pub gossip: Arc<GossipStore>,
    pub settlement: Arc<SettlementEngine>,
    pub db: PgPool,
}

// ── Peer discovery ────────────────────────────────────────────────────────────

/// POST /agent-swarm/peers
pub async fn register_peer(
    State(s): State<SwarmState>,
    Json(req): Json<RegisterPeerRequest>,
) -> impl IntoResponse {
    match s.discovery.register_peer(&req.peer_id, req.agent_id, &req.endpoint).await {
        Ok(peer) => (StatusCode::CREATED, Json(serde_json::json!(peer))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-swarm/peers
pub async fn list_peers(
    State(s): State<SwarmState>,
    Query(q): Query<PeerListQuery>,
) -> impl IntoResponse {
    match s.discovery.list_peers(&q).await {
        Ok(peers) => (StatusCode::OK, Json(serde_json::json!(peers))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// DELETE /agent-swarm/peers/:peer_id/revoke
pub async fn revoke_peer(
    State(s): State<SwarmState>,
    Path(peer_id): Path<String>,
) -> impl IntoResponse {
    match s.discovery.revoke_peer(&peer_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "revoked": peer_id }))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Task delegation ───────────────────────────────────────────────────────────

/// POST /agent-swarm/tasks
pub async fn create_swarm_task(
    State(s): State<SwarmState>,
    Json(req): Json<CreateSwarmTaskRequest>,
) -> impl IntoResponse {
    match s.delegation.create_and_delegate(req).await {
        Ok(task) => (StatusCode::CREATED, Json(serde_json::json!(task))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-swarm/tasks/:task_id
pub async fn get_swarm_task(
    State(s): State<SwarmState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.delegation.get_task(task_id).await {
        Ok(task) => (StatusCode::OK, Json(serde_json::json!(task))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-swarm/tasks/:task_id/micro-tasks
pub async fn list_micro_tasks(
    State(s): State<SwarmState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.delegation.list_micro_tasks(task_id).await {
        Ok(mts) => (StatusCode::OK, Json(serde_json::json!(mts))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// POST /agent-swarm/micro-tasks/:micro_task_id/submit
pub async fn submit_micro_task(
    State(s): State<SwarmState>,
    Path(micro_task_id): Path<Uuid>,
    Json(req): Json<SubmitMicroTaskRequest>,
) -> impl IntoResponse {
    match s.delegation.submit_micro_task(micro_task_id, req.agent_id, req.result_payload).await {
        Ok(mt) => (StatusCode::OK, Json(serde_json::json!(mt))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Consensus ─────────────────────────────────────────────────────────────────

/// POST /agent-swarm/tasks/:task_id/vote
pub async fn cast_vote(
    State(s): State<SwarmState>,
    Path(task_id): Path<Uuid>,
    Json(req): Json<CastVoteRequest>,
) -> impl IntoResponse {
    match s.consensus.cast_vote(task_id, req.voter_agent_id, &req.result_hash).await {
        Ok((vote, outcome)) => {
            // If consensus reached, auto-settle
            if outcome.reached {
                let task = s.delegation.get_task(task_id).await;
                if let Ok(t) = task {
                    let _ = s.settlement.settle_completed_task(task_id, t.manager_agent_id).await;
                    // Promote voters' reputation
                    if let Some(ref hash) = outcome.winning_hash {
                        let voters = sqlx::query!(
                            "SELECT v.voter_agent_id, p.peer_id \
                             FROM swarm_consensus_votes v \
                             JOIN swarm_peers p ON p.agent_id = v.voter_agent_id \
                             WHERE v.swarm_task_id = $1 AND v.result_hash = $2",
                            task_id,
                            hash,
                        )
                        .fetch_all(&s.db)
                        .await
                        .unwrap_or_default();
                        for voter in voters {
                            let _ = s.discovery.promote_peer(&voter.peer_id, 5).await;
                        }
                    }
                }
            }
            (StatusCode::OK, Json(serde_json::json!({ "vote": vote, "consensus": outcome }))).into_response()
        }
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-swarm/tasks/:task_id/votes
pub async fn list_votes(
    State(s): State<SwarmState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.consensus.list_votes(task_id).await {
        Ok(votes) => (StatusCode::OK, Json(serde_json::json!(votes))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Gossip ────────────────────────────────────────────────────────────────────

/// POST /agent-swarm/gossip
pub async fn gossip_push(
    State(s): State<SwarmState>,
    Json(req): Json<GossipPushRequest>,
) -> impl IntoResponse {
    match s.gossip.push(&req).await {
        Ok(entry) => (StatusCode::OK, Json(serde_json::json!(entry))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-swarm/gossip/snapshot
pub async fn gossip_snapshot(State(s): State<SwarmState>) -> impl IntoResponse {
    match s.gossip.snapshot().await {
        Ok(entries) => (StatusCode::OK, Json(serde_json::json!(entries))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// GET /agent-swarm/gossip/:key
pub async fn gossip_get(
    State(s): State<SwarmState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match s.gossip.get(&key).await {
        Ok(Some(entry)) => (StatusCode::OK, Json(serde_json::json!(entry))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "key not found" }))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── Settlement ────────────────────────────────────────────────────────────────

/// GET /agent-swarm/tasks/:task_id/settlements
pub async fn list_settlements(
    State(s): State<SwarmState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    match s.settlement.list_by_task(task_id).await {
        Ok(settlements) => (StatusCode::OK, Json(serde_json::json!(settlements))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// POST /agent-swarm/settlements/:settlement_id/confirm
pub async fn confirm_settlement(
    State(s): State<SwarmState>,
    Path(settlement_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let tx_hash = match body.get("stellar_tx_hash").and_then(|v| v.as_str()) {
        Some(h) => h.to_string(),
        None => return err("stellar_tx_hash required".to_string()).into_response(),
    };
    match s.settlement.confirm_settlement(settlement_id, &tx_hash).await {
        Ok(s) => (StatusCode::OK, Json(serde_json::json!(s))).into_response(),
        Err(e) => err(e).into_response(),
    }
}

// ── helper ────────────────────────────────────────────────────────────────────

fn err(msg: String) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "error": msg })))
}
