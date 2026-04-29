-- Agent Swarm Intelligence — Decentralized Coordination Layer
-- Issue: Swarm Intelligence / P2P Agent Coordination

CREATE TYPE peer_tier AS ENUM ('provisional', 'trusted', 'revoked');
CREATE TYPE swarm_task_status AS ENUM ('open', 'in_progress', 'completed', 'failed');

-- DHT-style peer routing table
CREATE TABLE swarm_peers (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    peer_id       TEXT NOT NULL UNIQUE,          -- SHA-256 of agent public key
    agent_id      UUID NOT NULL,                 -- references agent_registry(id)
    endpoint      TEXT NOT NULL,
    tier          peer_tier NOT NULL DEFAULT 'provisional',
    reputation    INT NOT NULL DEFAULT 50 CHECK (reputation BETWEEN 0 AND 100),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_swarm_peers_tier ON swarm_peers (tier, reputation DESC);
CREATE INDEX idx_swarm_peers_agent ON swarm_peers (agent_id);

-- Complex tasks broadcast by manager agents with cNGN bounties
CREATE TABLE swarm_task_requests (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_agent_id  UUID NOT NULL,
    description       TEXT NOT NULL,
    total_bounty_cngn NUMERIC(20, 6) NOT NULL,
    status            swarm_task_status NOT NULL DEFAULT 'open',
    required_votes    INT NOT NULL DEFAULT 1,
    result_payload    JSONB,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at      TIMESTAMPTZ
);

CREATE INDEX idx_swarm_tasks_manager ON swarm_task_requests (manager_agent_id, created_at DESC);
CREATE INDEX idx_swarm_tasks_status  ON swarm_task_requests (status) WHERE status IN ('open', 'in_progress');

-- Micro-tasks delegated to individual subordinate agents
CREATE TABLE swarm_micro_tasks (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swarm_task_id      UUID NOT NULL REFERENCES swarm_task_requests(id),
    assignee_agent_id  UUID NOT NULL,
    description        TEXT NOT NULL,
    bounty_cngn        NUMERIC(20, 6) NOT NULL,
    status             TEXT NOT NULL DEFAULT 'pending',
    result_payload     JSONB,
    submitted_at       TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_micro_tasks_swarm  ON swarm_micro_tasks (swarm_task_id);
CREATE INDEX idx_micro_tasks_agent  ON swarm_micro_tasks (assignee_agent_id, status);

-- Majority-voting consensus votes
CREATE TABLE swarm_consensus_votes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swarm_task_id   UUID NOT NULL REFERENCES swarm_task_requests(id),
    voter_agent_id  UUID NOT NULL,
    result_hash     TEXT NOT NULL,
    cast_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (swarm_task_id, voter_agent_id)
);

CREATE INDEX idx_votes_task ON swarm_consensus_votes (swarm_task_id, result_hash);

-- Gossip state — shared swarm intelligence (last-write-wins per key)
CREATE TABLE swarm_gossip_state (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    state_key       TEXT NOT NULL UNIQUE,
    value           JSONB NOT NULL,
    version         BIGINT NOT NULL DEFAULT 0,
    origin_peer_id  TEXT NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_gossip_updated ON swarm_gossip_state (updated_at DESC);

-- x402 on-chain settlement records for micro-task bounty payments
CREATE TABLE swarm_settlements (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swarm_task_id    UUID NOT NULL REFERENCES swarm_task_requests(id),
    micro_task_id    UUID NOT NULL REFERENCES swarm_micro_tasks(id),
    payer_agent_id   UUID NOT NULL,
    payee_agent_id   UUID NOT NULL,
    amount_cngn      NUMERIC(20, 6) NOT NULL,
    stellar_tx_hash  TEXT,
    status           TEXT NOT NULL DEFAULT 'pending',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at     TIMESTAMPTZ
);

CREATE INDEX idx_settlements_task   ON swarm_settlements (swarm_task_id);
CREATE INDEX idx_settlements_status ON swarm_settlements (status) WHERE status = 'pending';
