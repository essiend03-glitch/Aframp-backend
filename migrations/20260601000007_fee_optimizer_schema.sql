-- #490 Automated Gas & Fee Optimization Engine Schemas
-- Tables: network_fee_snapshots, fee_optimization_policies, execution_gas_logs

-- ── Network fee snapshots ─────────────────────────────────────────────────────

CREATE TYPE chain_network AS ENUM ('stellar', 'ethereum', 'solana', 'polygon', 'arbitrum');

CREATE TABLE network_fee_snapshots (
    snapshot_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    network             chain_network  NOT NULL,
    -- EVM: base_fee_wei, priority_fee_wei; Stellar: base_fee_stroops; Solana: compute_unit_price
    base_fee            NUMERIC(28,0)  NOT NULL,   -- native sub-unit (Wei / Stroop / Lamport)
    priority_fee        NUMERIC(28,0)  NOT NULL DEFAULT 0,
    -- EMA-smoothed values
    ema_base_fee        NUMERIC(28,0)  NOT NULL,
    ema_priority_fee    NUMERIC(28,0)  NOT NULL DEFAULT 0,
    -- RPC provider that supplied this snapshot
    rpc_provider        VARCHAR(64)    NOT NULL,
    -- Block number or ledger sequence at time of snapshot
    block_reference     BIGINT,
    captured_at         TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fee_snapshots_network ON network_fee_snapshots(network, captured_at DESC);
CREATE INDEX idx_fee_snapshots_time    ON network_fee_snapshots(captured_at DESC);

-- ── Fee optimization policies ─────────────────────────────────────────────────

CREATE TYPE urgency_window AS ENUM ('immediate', 'one_min', 'five_min', 'thirty_min', 'best_effort');

CREATE TABLE fee_optimization_policies (
    policy_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- NULL = platform-wide default; non-NULL = tenant-specific override
    tenant_id           UUID,
    network             chain_network  NOT NULL,
    urgency             urgency_window NOT NULL,
    -- Maximum fee cap in native sub-units
    max_fee_cap         NUMERIC(28,0)  NOT NULL,
    -- Multiplier applied to EMA for priority fee calculation (e.g. 1.20 = 20 % above EMA)
    fee_multiplier      NUMERIC(6,4)   NOT NULL DEFAULT 1.10,
    -- Halt batch payouts if fee exceeds this threshold
    congestion_halt_threshold NUMERIC(28,0) NOT NULL,
    enabled             BOOLEAN        NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, network, urgency)
);

CREATE INDEX idx_fee_policies_network ON fee_optimization_policies(network);

-- ── Execution gas logs ────────────────────────────────────────────────────────

CREATE TYPE gas_log_status AS ENUM (
    'pending', 'submitted', 'confirmed', 'bumped', 'dropped', 'failed'
);

CREATE TABLE execution_gas_logs (
    gas_log_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Originating settlement or rebalancing transaction
    parent_tx_id        UUID           NOT NULL,
    network             chain_network  NOT NULL,
    urgency             urgency_window NOT NULL,
    -- Estimated fee at submission time
    estimated_fee       NUMERIC(28,0)  NOT NULL,
    -- Actual fee paid (filled after confirmation)
    actual_fee          NUMERIC(28,0),
    -- Over/under estimation delta
    fee_delta           NUMERIC(28,0)  GENERATED ALWAYS AS (actual_fee - estimated_fee) STORED,
    -- Number of fee-bump replacements issued
    bump_count          INTEGER        NOT NULL DEFAULT 0,
    -- Transaction hash / sequence on the target chain
    tx_hash             VARCHAR(128),
    nonce_or_sequence   BIGINT,
    status              gas_log_status NOT NULL DEFAULT 'pending',
    submitted_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    confirmed_at        TIMESTAMPTZ,
    last_bumped_at      TIMESTAMPTZ
);

CREATE INDEX idx_gas_logs_parent  ON execution_gas_logs(parent_tx_id);
CREATE INDEX idx_gas_logs_network ON execution_gas_logs(network, status);
CREATE INDEX idx_gas_logs_pending ON execution_gas_logs(submitted_at)
    WHERE status IN ('pending', 'submitted', 'bumped');

-- Seed default platform-wide policies
INSERT INTO fee_optimization_policies
    (tenant_id, network, urgency, max_fee_cap, fee_multiplier, congestion_halt_threshold)
VALUES
    (NULL, 'stellar',  'immediate',   500,          1.20, 1000),
    (NULL, 'stellar',  'five_min',    200,          1.05, 1000),
    (NULL, 'ethereum', 'immediate',   100000000000, 1.30, 500000000000),
    (NULL, 'ethereum', 'five_min',    50000000000,  1.10, 500000000000),
    (NULL, 'ethereum', 'thirty_min',  20000000000,  1.00, 500000000000),
    (NULL, 'solana',   'immediate',   1000000,      1.20, 5000000),
    (NULL, 'solana',   'five_min',    500000,       1.05, 5000000);
