-- Issue #533: Decentralized Oracles, BFT Price Feeds & MEV Protection
-- ─────────────────────────────────────────────────────────────────────────────

-- Oracle provider node registry
CREATE TABLE IF NOT EXISTS oracle_provider_nodes (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name             TEXT NOT NULL UNIQUE,
    endpoint_url     TEXT NOT NULL,
    public_key_ed25519 TEXT NOT NULL,
    consensus_weight INT  NOT NULL DEFAULT 1,
    sla_status       TEXT NOT NULL DEFAULT 'active',   -- 'active' | 'degraded' | 'offline'
    accuracy_score   NUMERIC(5,4) NOT NULL DEFAULT 1.0,
    enabled          BOOL NOT NULL DEFAULT TRUE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- TimescaleDB hypertable: raw oracle price ticks at 100ms granularity
CREATE TABLE IF NOT EXISTS oracle_price_ticks_historical (
    id          BIGSERIAL,
    node_id     UUID        NOT NULL,
    pair        TEXT        NOT NULL,
    price       NUMERIC(40,18) NOT NULL,
    tick_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, tick_at)
) PARTITION BY RANGE (tick_at);

CREATE TABLE IF NOT EXISTS oracle_price_ticks_historical_default
    PARTITION OF oracle_price_ticks_historical DEFAULT;

CREATE INDEX IF NOT EXISTS idx_opth_pair_time
    ON oracle_price_ticks_historical (pair, tick_at DESC);

-- MEV front-running interception log
CREATE TABLE IF NOT EXISTS mev_frontrunning_interceptions (
    id              BIGSERIAL PRIMARY KEY,
    pair            TEXT NOT NULL,
    detected_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    spot_price      NUMERIC(40,18) NOT NULL,
    baseline_price  NUMERIC(40,18) NOT NULL,
    deviation_pct   NUMERIC(10,6)  NOT NULL,
    action_taken    TEXT NOT NULL,                     -- 'PAUSED' | 'DELAYED' | 'PRIVATE_ROUTE'
    tenant_id       UUID
);

CREATE INDEX IF NOT EXISTS idx_mfi_pair_time
    ON mev_frontrunning_interceptions (pair, detected_at DESC);
