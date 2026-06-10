-- Issue #530: Multi-Tenant Resource Isolation, Rate Limiting & Fair-Share Scheduling
-- ─────────────────────────────────────────────────────────────────────────────

-- Tenant SLA profiles with resource entitlements
CREATE TABLE IF NOT EXISTS tenant_sla_profiles (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id            UUID NOT NULL UNIQUE,
    tier                 TEXT NOT NULL DEFAULT 'standard',
    max_concurrent_conns INT  NOT NULL DEFAULT 50,
    baseline_rps         INT  NOT NULL DEFAULT 100,
    burst_rps            INT  NOT NULL DEFAULT 200,
    queue_weight         INT  NOT NULL DEFAULT 10,
    burst_window_ms      INT  NOT NULL DEFAULT 5000,
    enabled              BOOL NOT NULL DEFAULT TRUE,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Rolling time-series ledger for throughput & rate-limit infractions
CREATE TABLE IF NOT EXISTS resource_consumption_ledger (
    id              BIGSERIAL,
    tenant_id       UUID        NOT NULL,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    rps_observed    INT         NOT NULL DEFAULT 0,
    tokens_consumed INT         NOT NULL DEFAULT 0,
    throttled       BOOL        NOT NULL DEFAULT FALSE,
    infraction_type TEXT,
    corridor_id     UUID,
    PRIMARY KEY (id, recorded_at)
) PARTITION BY RANGE (recorded_at);

CREATE TABLE IF NOT EXISTS resource_consumption_ledger_default
    PARTITION OF resource_consumption_ledger DEFAULT;

CREATE INDEX IF NOT EXISTS idx_rcl_tenant_time
    ON resource_consumption_ledger (tenant_id, recorded_at DESC);

-- Corridor retry buckets for queue evacuation (head-of-line blocking prevention)
CREATE TABLE IF NOT EXISTS corridor_retry_buckets (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID        NOT NULL,
    corridor_id UUID        NOT NULL,
    reason      TEXT        NOT NULL,
    queued_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    retry_after TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '60 seconds',
    attempts    INT         NOT NULL DEFAULT 0,
    resolved    BOOL        NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_crb_pending
    ON corridor_retry_buckets (tenant_id, corridor_id) WHERE NOT resolved;
