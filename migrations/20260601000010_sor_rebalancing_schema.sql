-- #487 Smart Order Routing & Treasury Rebalancing Schemas
-- Tables: liquidity_venues, smart_order_executions, treasury_rebalancing_rules

-- ── Liquidity venues ──────────────────────────────────────────────────────────

CREATE TYPE venue_type AS ENUM ('regional_bank', 'stellar_amm', 'mto', 'cex', 'dex');
CREATE TYPE venue_status AS ENUM ('active', 'degraded', 'offline', 'suspended');

CREATE TABLE liquidity_venues (
    venue_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                VARCHAR(120)   NOT NULL,
    venue_type          venue_type     NOT NULL,
    status              venue_status   NOT NULL DEFAULT 'active',
    api_endpoint        TEXT           NOT NULL,
    -- Encrypted credentials stored as opaque blob; decrypted at runtime via KMS
    api_credentials     BYTEA,
    supported_currencies TEXT[]        NOT NULL DEFAULT '{}',
    daily_volume_limit  NUMERIC(28,7)  NOT NULL DEFAULT 0,
    used_volume_today   NUMERIC(28,7)  NOT NULL DEFAULT 0,
    execution_fee_bps   NUMERIC(10,4)  NOT NULL DEFAULT 0,   -- basis points
    spread_bps          NUMERIC(10,4)  NOT NULL DEFAULT 0,
    last_heartbeat_at   TIMESTAMPTZ,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_venues_status ON liquidity_venues(status);
CREATE INDEX idx_venues_type   ON liquidity_venues(venue_type);

-- ── Smart order executions ────────────────────────────────────────────────────

CREATE TYPE sor_status AS ENUM (
    'pending', 'routing', 'partial', 'completed', 'failed', 'rolled_back'
);

CREATE TABLE smart_order_executions (
    execution_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Correlation tag linking all child orders to the originating remittance
    parent_transaction_id UUID NOT NULL,
    correlation_tag     VARCHAR(64)    NOT NULL,
    source_currency     VARCHAR(10)    NOT NULL,
    target_currency     VARCHAR(10)    NOT NULL,
    total_amount        NUMERIC(28,7)  NOT NULL,
    status              sor_status     NOT NULL DEFAULT 'pending',
    -- Routing metadata (JSON array of child order descriptors)
    routing_plan        JSONB          NOT NULL DEFAULT '[]',
    -- Slippage actually observed across all child fills
    realized_slippage_bps NUMERIC(10,4),
    -- Computational window for path selection (ms)
    path_calc_ms        INTEGER,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ
);

CREATE INDEX idx_sor_parent_tx  ON smart_order_executions(parent_transaction_id);
CREATE INDEX idx_sor_correlation ON smart_order_executions(correlation_tag);
CREATE INDEX idx_sor_status      ON smart_order_executions(status);

-- Child order rows (one per venue slice)
CREATE TYPE child_order_status AS ENUM (
    'pending', 'submitted', 'filled', 'partial_fill', 'failed', 'timed_out'
);

CREATE TABLE sor_child_orders (
    child_order_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_id        UUID           NOT NULL REFERENCES smart_order_executions(execution_id),
    venue_id            UUID           NOT NULL REFERENCES liquidity_venues(venue_id),
    allocation_pct      NUMERIC(6,4)   NOT NULL,   -- e.g. 0.4000 = 40 %
    allocated_amount    NUMERIC(28,7)  NOT NULL,
    filled_amount       NUMERIC(28,7)  NOT NULL DEFAULT 0,
    status              child_order_status NOT NULL DEFAULT 'pending',
    venue_order_ref     VARCHAR(128),
    slippage_bps        NUMERIC(10,4),
    submitted_at        TIMESTAMPTZ,
    filled_at           TIMESTAMPTZ,
    failed_reason       TEXT
);

CREATE INDEX idx_child_execution ON sor_child_orders(execution_id);
CREATE INDEX idx_child_venue      ON sor_child_orders(venue_id);

-- ── Treasury rebalancing rules ────────────────────────────────────────────────

CREATE TYPE rebalancing_trigger AS ENUM ('threshold_breach', 'scheduled', 'manual');

CREATE TABLE treasury_rebalancing_rules (
    rule_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    currency_code       VARCHAR(10)    NOT NULL UNIQUE,
    -- Minimum inventory fraction (e.g. 0.20 = 20 %)
    min_inventory_pct   NUMERIC(6,4)   NOT NULL,
    -- Target inventory fraction
    target_inventory_pct NUMERIC(6,4)  NOT NULL,
    -- Hard cap fraction
    max_inventory_pct   NUMERIC(6,4)   NOT NULL,
    trigger_type        rebalancing_trigger NOT NULL DEFAULT 'threshold_breach',
    -- Cron expression for scheduled triggers (NULL for threshold-only)
    schedule_cron       VARCHAR(64),
    enabled             BOOLEAN        NOT NULL DEFAULT TRUE,
    last_triggered_at   TIMESTAMPTZ,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_rebalancing_pcts
        CHECK (min_inventory_pct <= target_inventory_pct
               AND target_inventory_pct <= max_inventory_pct)
);

-- Rebalancing execution log
CREATE TYPE rebalance_status AS ENUM ('initiated', 'in_progress', 'completed', 'failed');

CREATE TABLE treasury_rebalancing_log (
    log_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id             UUID           NOT NULL REFERENCES treasury_rebalancing_rules(rule_id),
    currency_code       VARCHAR(10)    NOT NULL,
    trigger_type        rebalancing_trigger NOT NULL,
    amount_rebalanced   NUMERIC(28,7)  NOT NULL DEFAULT 0,
    status              rebalance_status NOT NULL DEFAULT 'initiated',
    stellar_tx_hash     VARCHAR(128),
    redis_lock_key      VARCHAR(128),
    error_message       TEXT,
    initiated_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ
);

CREATE INDEX idx_rebalance_log_rule     ON treasury_rebalancing_log(rule_id);
CREATE INDEX idx_rebalance_log_currency ON treasury_rebalancing_log(currency_code);
CREATE INDEX idx_rebalance_log_status   ON treasury_rebalancing_log(status);

-- Seed default rebalancing rules for major corridors
INSERT INTO treasury_rebalancing_rules
    (currency_code, min_inventory_pct, target_inventory_pct, max_inventory_pct)
VALUES
    ('NGN',  0.20, 0.30, 0.50),
    ('KES',  0.15, 0.25, 0.45),
    ('USDC', 0.10, 0.20, 0.40),
    ('GHS',  0.10, 0.20, 0.40),
    ('ZAR',  0.10, 0.20, 0.40);
