-- Migration: Cryptographic Audit Trail & Proof of Reserves (PoR) — Issue #297
--
-- Creates the tables that back the automated PoR service:
--   * audit_snapshots             — hourly PoR snapshots (supply + aggregated fiat balances + ratio)
--   * merkle_proof_registry       — stores compiled Merkle root hashes, block heights, and anchoring transaction details
--   * fiat_bank_balances_historical — time-series partitioned table for signed balance statements

-- ── 1. Audit Snapshots ────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS audit_snapshots (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- On-chain cNGN circulating supply + AMM reserves
    total_on_chain_supply   NUMERIC(36, 8) NOT NULL CHECK (total_on_chain_supply >= 0),
    -- Sum of all custodian bank settled balances in NGN
    total_fiat_balances     NUMERIC(36, 8) NOT NULL CHECK (total_fiat_balances >= 0),
    -- reserve_backing_ratio = total_fiat_balances / total_on_chain_supply
    reserve_backing_ratio   NUMERIC(12, 6) NOT NULL CHECK (reserve_backing_ratio >= 0),
    recorded_at             TIMESTAMPTZ    NOT NULL DEFAULT now(),
    created_at              TIMESTAMPTZ    NOT NULL DEFAULT now()
);

COMMENT ON TABLE audit_snapshots IS
    'Hourly snapshots tracking outstanding cNGN supply versus aggregated fiat bank balances.';

CREATE INDEX IF NOT EXISTS idx_audit_snapshots_recorded_at
    ON audit_snapshots (recorded_at DESC);

-- ── 2. Merkle Proof Registry ──────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS merkle_proof_registry (
    id                         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id                UUID        NOT NULL REFERENCES audit_snapshots(id) ON DELETE CASCADE,
    merkle_root_hash           TEXT        NOT NULL,
    block_confirmation_height  BIGINT,
    anchoring_tx_signature     TEXT,
    tree_depth                 INT         NOT NULL,
    recorded_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at                 TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE merkle_proof_registry IS
    'Registry of compiled Merkle roots anchored onto the Stellar blockchain.';

CREATE INDEX IF NOT EXISTS idx_merkle_proof_registry_snapshot
    ON merkle_proof_registry (snapshot_id);
CREATE INDEX IF NOT EXISTS idx_merkle_proof_registry_root
    ON merkle_proof_registry (merkle_root_hash);

-- ── 3. Fiat Bank Balances Historical ──────────────────────────────────────────
-- Partitions are used for time-series isolation.
CREATE TABLE IF NOT EXISTS fiat_bank_balances_historical (
    id              UUID        NOT NULL DEFAULT gen_random_uuid(),
    bank_label      TEXT        NOT NULL,
    settled_balance NUMERIC(36, 8) NOT NULL CHECK (settled_balance >= 0),
    currency        TEXT        NOT NULL DEFAULT 'NGN',
    signature       TEXT        NOT NULL,
    signing_key     TEXT        NOT NULL,
    balance_as_of   TIMESTAMPTZ NOT NULL,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, recorded_at)
) PARTITION BY RANGE (recorded_at);

COMMENT ON TABLE fiat_bank_balances_historical IS
    'Historical time-series partitioned log of cryptographically signed custodian bank balance statements.';

-- Create partitions for 2026, 2027, 2028, and a default partition
CREATE TABLE IF NOT EXISTS fiat_bank_balances_historical_y2026 PARTITION OF fiat_bank_balances_historical
    FOR VALUES FROM ('2026-01-01 00:00:00+00') TO ('2027-01-01 00:00:00+00');

CREATE TABLE IF NOT EXISTS fiat_bank_balances_historical_y2027 PARTITION OF fiat_bank_balances_historical
    FOR VALUES FROM ('2027-01-01 00:00:00+00') TO ('2028-01-01 00:00:00+00');

CREATE TABLE IF NOT EXISTS fiat_bank_balances_historical_y2028 PARTITION OF fiat_bank_balances_historical
    FOR VALUES FROM ('2028-01-01 00:00:00+00') TO ('2029-01-01 00:00:00+00');

CREATE TABLE IF NOT EXISTS fiat_bank_balances_historical_default PARTITION OF fiat_bank_balances_historical DEFAULT;

CREATE INDEX IF NOT EXISTS idx_fiat_bank_balances_historical_recorded_at
    ON fiat_bank_balances_historical (recorded_at DESC);
