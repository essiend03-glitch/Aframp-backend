use crate::agent_swarm::handlers::{
    cast_vote, confirm_settlement, create_swarm_task, get_swarm_task, gossip_get, gossip_push,
    gossip_snapshot, list_micro_tasks, list_peers, list_settlements, list_votes, register_peer,
    revoke_peer, submit_micro_task, SwarmState,
};
use axum::{routing::{delete, get, post}, Router};

pub fn agent_swarm_routes(state: SwarmState) -> Router {
    Router::new()
        // Peer discovery
        .route("/agent-swarm/peers", post(register_peer).get(list_peers))
        .route("/agent-swarm/peers/:peer_id/revoke", delete(revoke_peer))
        // Task delegation
        .route("/agent-swarm/tasks", post(create_swarm_task))
        .route("/agent-swarm/tasks/:task_id", get(get_swarm_task))
        .route("/agent-swarm/tasks/:task_id/micro-tasks", get(list_micro_tasks))
        .route("/agent-swarm/micro-tasks/:micro_task_id/submit", post(submit_micro_task))
        // Consensus / voting
        .route("/agent-swarm/tasks/:task_id/vote", post(cast_vote))
        .route("/agent-swarm/tasks/:task_id/votes", get(list_votes))
        // Gossip state
        .route("/agent-swarm/gossip", post(gossip_push))
        .route("/agent-swarm/gossip/snapshot", get(gossip_snapshot))
        .route("/agent-swarm/gossip/:key", get(gossip_get))
        // Settlements
        .route("/agent-swarm/tasks/:task_id/settlements", get(list_settlements))
        .route("/agent-swarm/settlements/:settlement_id/confirm", post(confirm_settlement))
        .with_state(state)
}
