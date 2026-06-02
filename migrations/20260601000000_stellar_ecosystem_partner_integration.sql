-- Stellar Ecosystem Partner Integration (Issue #470)
-- Tables: stellar_anchor_connections, dex_order_book_snapshots, cross_anchor_transfers

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. stellar_anchor_connections
--    Manages partner anchor domains, verified asset codes, and SEP enablement.
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TYPE anchor_status AS ENUM ('active', 'suspended', 'pending_verification');

CREATE TABLE IF NOT EXISTS stellar_anchor_connections (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- e.g. "anchor.example.com"
    domain              TEXT        NOT NULL UNIQUE,
    display_name        TEXT        NOT NULL,
    status              anchor_status NOT NULL DEFAULT 'pending_verification',
    -- Verified asset codes this anchor supports, e.g. '{"USDC","EURC"}'
    supported_assets    TEXT[]      NOT NULL DEFAULT '{}',
    -- Which SEP protocols are enabled for this anchor
    sep24_enabled       BOOLEAN     NOT NULL DEFAULT FALSE,
    sep31_enabled       BOOLEAN     NOT NULL DEFAULT FALSE,
    -- stellar.toml SIGNING_KEY for this anchor (public key)
    signing_key         TEXT,
    -- JWT auth token (encrypted at rest by application layer)
    jwt_token           TEXT,
    jwt_expires_at      TIMESTAMPTZ,
    -- Horizon base URL override (null = use platform default)
    horizon_url         TEXT,
    -- Cumulative stats
    total_transfers     BIGINT      NOT NULL DEFAULT 0,
    total_volume_usd    NUMERIC(20,7) NOT NULL DEFAULT 0,
    last_connected_at   TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_anchor_connections_status  ON stellar_anchor_connections (status);
CREATE INDEX idx_anchor_connections_domain  ON stellar_anchor_connections (domain);

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. dex_order_book_snapshots
--    Local cache of order-book depth for monitored asset pairs.
--    Used to optimise pathfinding without hitting Horizon on every request.
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS dex_order_book_snapshots (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- e.g. "cNGN:GISSUER.../USDC:GCIRCLE..."
    base_asset      TEXT        NOT NULL,
    counter_asset   TEXT        NOT NULL,
    -- Best bid / ask prices (7 decimal places — Stellar native precision)
    best_bid        NUMERIC(24,7),
    best_ask        NUMERIC(24,7),
    mid_price       NUMERIC(24,7),
    spread_pct      NUMERIC(10,7),
    -- Full order-book depth as JSON (bids/asks arrays)
    bids            JSONB       NOT NULL DEFAULT '[]',
    asks            JSONB       NOT NULL DEFAULT '[]',
    -- Total liquidity depth within 1% of mid-price
    depth_1pct_base    NUMERIC(24,7) NOT NULL DEFAULT 0,
    depth_1pct_counter NUMERIC(24,7) NOT NULL DEFAULT 0,
    snapshotted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- TTL: application discards rows older than this
    expires_at      TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '5 minutes')
);

CREATE INDEX idx_dex_snapshots_pair       ON dex_order_book_snapshots (base_asset, counter_asset);
CREATE INDEX idx_dex_snapshots_expires_at ON dex_order_book_snapshots (expires_at);
-- Partial index: only live snapshots
CREATE INDEX idx_dex_snapshots_live       ON dex_order_book_snapshots (base_asset, counter_asset, snapshotted_at DESC)
    WHERE expires_at > NOW();

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. cross_anchor_transfers
--    End-to-end lifecycle of SEP-31 cross-border asset swaps.
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TYPE cross_anchor_status AS ENUM (
    'initiated',
    'pending_sender',
    'pending_stellar',
    'pending_receiver',
    'pending_external',
    'completed',
    'refunded',
    'expired',
    'error'
);

CREATE TABLE IF NOT EXISTS cross_anchor_transfers (
    id                      UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Internal reference
    reference_id            TEXT            NOT NULL UNIQUE DEFAULT gen_random_uuid()::TEXT,
    -- Anchor that receives the payment (FK to stellar_anchor_connections)
    receiving_anchor_id     UUID            NOT NULL REFERENCES stellar_anchor_connections(id),
    -- SEP-31 transaction ID returned by the receiving anchor
    sep31_transaction_id    TEXT            UNIQUE,
    -- Compliance payload tracking ID (SEP-12 customer info reference)
    compliance_tracking_id  TEXT,
    status                  cross_anchor_status NOT NULL DEFAULT 'initiated',
    -- Asset being sent (e.g. "cNGN:GISSUER...")
    send_asset              TEXT            NOT NULL,
    -- Asset received by beneficiary (e.g. "USDC:GCIRCLE...")
    receive_asset           TEXT            NOT NULL,
    -- Amounts with 7 decimal precision (Stellar native)
    send_amount             NUMERIC(24,7)   NOT NULL,
    receive_amount          NUMERIC(24,7),
    -- Execution spread at time of submission (fraction, e.g. 0.0025)
    execution_spread        NUMERIC(10,7),
    -- Stellar transaction hash once submitted on-chain
    stellar_tx_hash         TEXT,
    -- Base64-encoded XDR of the submitted transaction
    stellar_tx_xdr          TEXT,
    -- Stellar ledger sequence number
    stellar_ledger          BIGINT,
    -- Sender / receiver Stellar accounts
    sender_account          TEXT            NOT NULL,
    receiver_account        TEXT,
    -- Error detail if status = 'error'
    error_code              TEXT,
    error_message           TEXT,
    -- Timestamps
    submitted_at            TIMESTAMPTZ,
    completed_at            TIMESTAMPTZ,
    expires_at              TIMESTAMPTZ     NOT NULL DEFAULT (NOW() + INTERVAL '24 hours'),
    created_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cross_anchor_status          ON cross_anchor_transfers (status);
CREATE INDEX idx_cross_anchor_anchor_id       ON cross_anchor_transfers (receiving_anchor_id);
CREATE INDEX idx_cross_anchor_stellar_tx_hash ON cross_anchor_transfers (stellar_tx_hash) WHERE stellar_tx_hash IS NOT NULL;
CREATE INDEX idx_cross_anchor_created_at      ON cross_anchor_transfers (created_at DESC);
CREATE INDEX idx_cross_anchor_sep31_id        ON cross_anchor_transfers (sep31_transaction_id) WHERE sep31_transaction_id IS NOT NULL;

-- Auto-update updated_at
CREATE OR REPLACE FUNCTION update_cross_anchor_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$;

CREATE TRIGGER trg_cross_anchor_updated_at
    BEFORE UPDATE ON cross_anchor_transfers
    FOR EACH ROW EXECUTE FUNCTION update_cross_anchor_updated_at();

CREATE OR REPLACE FUNCTION update_anchor_connection_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$;

CREATE TRIGGER trg_anchor_connection_updated_at
    BEFORE UPDATE ON stellar_anchor_connections
    FOR EACH ROW EXECUTE FUNCTION update_anchor_connection_updated_at();
