-- Agent Admin Dashboard — HITL Control System
-- Issue: Agent Admin Dashboard (#5.05)

CREATE TYPE agent_op_status AS ENUM ('idle', 'working', 'negotiating', 'paused', 'error');

-- Central agent registry with live telemetry snapshot
CREATE TABLE agent_registry (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                 TEXT NOT NULL,
    template_id          UUID,
    op_status            agent_op_status NOT NULL DEFAULT 'idle',
    burn_rate_cngn       NUMERIC(20, 6) NOT NULL DEFAULT 0,
    spent_today_cngn     NUMERIC(20, 6) NOT NULL DEFAULT 0,
    daily_limit_cngn     NUMERIC(20, 6) NOT NULL DEFAULT 100,
    wallet_balance_cngn  NUMERIC(20, 6) NOT NULL DEFAULT 0,
    tasks_completed      BIGINT NOT NULL DEFAULT 0,
    tasks_failed         BIGINT NOT NULL DEFAULT 0,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_registry_status ON agent_registry (op_status);

-- Tasks with reasoning trace (JSONB array of thought steps)
CREATE TABLE agent_tasks (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id             UUID NOT NULL REFERENCES agent_registry(id),
    description          TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'queued',
    projected_cost_cngn  NUMERIC(20, 6) NOT NULL DEFAULT 0,
    actual_cost_cngn     NUMERIC(20, 6),
    reasoning_trace      JSONB NOT NULL DEFAULT '[]',
    started_at           TIMESTAMPTZ,
    completed_at         TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_tasks_agent ON agent_tasks (agent_id, created_at DESC);
CREATE INDEX idx_agent_tasks_status ON agent_tasks (status) WHERE status IN ('running', 'awaiting_approval');

-- Human Approval Queue — tasks flagged as high-risk or over-budget
CREATE TABLE agent_approval_queue (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id             UUID NOT NULL REFERENCES agent_registry(id),
    task_id              UUID NOT NULL REFERENCES agent_tasks(id),
    reason               TEXT NOT NULL,
    projected_cost_cngn  NUMERIC(20, 6) NOT NULL,
    decision             TEXT NOT NULL DEFAULT 'pending',
    decided_by           TEXT,
    decided_at           TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_approval_queue_pending ON agent_approval_queue (created_at ASC)
    WHERE decision = 'pending';

-- Manual intervention log (pause / reset / circuit_breaker / authorize / resume)
CREATE TABLE agent_intervention_logs (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id      UUID NOT NULL REFERENCES agent_registry(id),
    action        TEXT NOT NULL,
    performed_by  TEXT NOT NULL,
    reason        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_intervention_logs_agent ON agent_intervention_logs (agent_id, created_at DESC);

-- Agent templates — versioned instruction sets for swarm management
CREATE TABLE agent_templates (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT NOT NULL,
    instructions TEXT NOT NULL,
    version      INT NOT NULL DEFAULT 1,
    created_by   TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
