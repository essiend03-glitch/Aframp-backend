-- Agent CFO: In-House Treasury Management for Autonomous Agents
-- Issue: Agentic Treasury / In-House CFO

CREATE TYPE llm_tier AS ENUM ('advanced', 'efficient');
CREATE TYPE agent_key_state AS ENUM ('active', 'frozen');

-- Budget policy per agent
CREATE TABLE agent_budget_policies (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id            UUID NOT NULL UNIQUE,
    daily_limit_cngn    NUMERIC(20, 6) NOT NULL DEFAULT 100,
    task_limit_cngn     NUMERIC(20, 6) NOT NULL DEFAULT 500,
    safety_buffer_cngn  NUMERIC(20, 6) NOT NULL DEFAULT 20,
    refill_amount_cngn  NUMERIC(20, 6) NOT NULL DEFAULT 50,
    funding_account     TEXT NOT NULL,
    llm_tier            llm_tier NOT NULL DEFAULT 'advanced',
    key_state           agent_key_state NOT NULL DEFAULT 'active',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Expenditure ledger: every inference / API call event
CREATE TABLE agent_inference_events (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id                UUID NOT NULL REFERENCES agent_budget_policies(agent_id),
    task_id                 UUID,
    event_type              TEXT NOT NULL,          -- 'llm_call' | 'api_request' | 'data_query'
    model_used              TEXT,
    cost_cngn               NUMERIC(20, 6) NOT NULL,
    cumulative_daily_spend  NUMERIC(20, 6) NOT NULL,
    metadata                JSONB NOT NULL DEFAULT '{}',
    recorded_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_inference_agent_day
    ON agent_inference_events (agent_id, recorded_at DESC);

CREATE INDEX idx_inference_task
    ON agent_inference_events (task_id)
    WHERE task_id IS NOT NULL;

-- Agent wallets (balance tracking for refill trigger)
CREATE TABLE agent_wallets (
    agent_id        UUID PRIMARY KEY REFERENCES agent_budget_policies(agent_id),
    balance_cngn    NUMERIC(20, 6) NOT NULL DEFAULT 0,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Refill requests queued when balance drops below safety buffer
CREATE TABLE agent_refill_requests (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id            UUID NOT NULL REFERENCES agent_budget_policies(agent_id),
    refill_amount_cngn  NUMERIC(20, 6) NOT NULL,
    funding_account     TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'pending', -- 'pending' | 'completed' | 'failed'
    requested_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ,
    UNIQUE (agent_id, requested_at)
);

-- Budget update reports (50% threshold notifications)
CREATE TABLE agent_budget_reports (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id            UUID NOT NULL REFERENCES agent_budget_policies(agent_id),
    daily_limit_cngn    NUMERIC(20, 6) NOT NULL,
    spent_today_cngn    NUMERIC(20, 6) NOT NULL,
    spend_percent       DOUBLE PRECISION NOT NULL,
    llm_tier            llm_tier NOT NULL,
    key_state           agent_key_state NOT NULL,
    reported_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_budget_reports_agent
    ON agent_budget_reports (agent_id, reported_at DESC);
