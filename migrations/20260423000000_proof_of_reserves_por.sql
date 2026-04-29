-- Migration: Stablecoin Reserve Management & Proof-of-Reserves (PoR) — Issue #297
--
-- Creates the tables that back the automated PoR service:
--   * por_snapshots          — hourly PoR snapshots (supply + bank reserves + ratio)
--   * por_bank_balances      — per-bank settled balances for each snapshot
--   * por_discrepancy_alerts — investigation alerts when ratio deviates > 0.05%

-- ── PoR Snapshots ─────────────────────────────────────────────────────────────
-- Populated by the PoR worker every 60 minutes.
-- The public /v1/transparency/por endpoint always reads the most-recent row.
CREATE TABLE IF NOT EXISTS por_snapshots (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- On-chain cNGN circulating supply (Total Minted − Total Burned)
    total_on_chain_supply   NUMERIC(36, 8) NOT NULL CHECK (total_on_chain_supply >= 0),
    -- Sum of all custodian bank settled balances in NGN
    total_bank_assets       NUMERIC(36, 8) NOT NULL CHECK (total_bank_assets >= 0),
    -- (total_bank_assets / total_on_chain_supply) * 100
    collateralization_ratio NUMERIC(12, 6) NOT NULL CHECK (collateralization_ratio >= 0),
    -- true when ratio >= 100.01 (fully collateralised)
    is_fully_collateralized BOOLEAN        NOT NULL DEFAULT false,
    -- ISO-8601 timestamp provided by the custodian bank (Proof of Solvency)
    custodian_solvency_ts   TIMESTAMPTZ    NOT NULL,
    -- Ed25519 signature over the canonical payload
    signature               TEXT           NOT NULL,
    -- Hex-encoded Ed25519 public key
    signing_key             TEXT           NOT NULL,
    recorded_at             TIMESTAMPTZ    NOT NULL DEFAULT now(),
    created_at              TIMESTAMPTZ    NOT NULL DEFAULT now()
);

COMMENT ON TABLE por_snapshots IS
    'Hourly Proof-of-Reserves snapshots linking cNGN on-chain supply to NGN custodian bank balances.';
COMMENT ON COLUMN por_snapshots.collateralization_ratio IS
    'Ratio expressed as a percentage: 100.00 = fully backed 1:1.';
COMMENT ON COLUMN por_snapshots.custodian_solvency_ts IS
    'Timestamp from the custodian bank confirming the settled balance (Proof of Solvency).';

CREATE INDEX IF NOT EXISTS idx_por_snapshots_recorded_at
    ON por_snapshots (recorded_at DESC);

-- ── Per-bank Balances ─────────────────────────────────────────────────────────
-- Anonymised per-bank settled balances attached to each snapshot.
CREATE TABLE IF NOT EXISTS por_bank_balances (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id     UUID        NOT NULL REFERENCES por_snapshots(id) ON DELETE CASCADE,
    -- Anonymised label, e.g. "Reserve Vault A"
    bank_label      TEXT        NOT NULL,
    -- Settled balance in NGN (read-only credential, cannot move funds)
    settled_balance NUMERIC(36, 8) NOT NULL CHECK (settled_balance >= 0),
    currency        TEXT        NOT NULL DEFAULT 'NGN',
    -- ISO-8601 timestamp from the bank API confirming the balance
    balance_as_of   TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_por_bank_balances_snapshot
    ON por_bank_balances (snapshot_id);

-- ── Discrepancy Alerts ────────────────────────────────────────────────────────
-- Investigation alerts raised when the collateralization ratio deviates > 0.05%
-- from 100.00% (i.e. ratio < 99.95% or ratio < 100.01% per issue spec).
CREATE TABLE IF NOT EXISTS por_discrepancy_alerts (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id             UUID        NOT NULL REFERENCES por_snapshots(id) ON DELETE CASCADE,
    collateralization_ratio NUMERIC(12, 6) NOT NULL,
    total_on_chain_supply   NUMERIC(36, 8) NOT NULL,
    total_bank_assets       NUMERIC(36, 8) NOT NULL,
    -- Absolute shortfall (supply − bank_assets), positive means under-collateralised
    shortfall               NUMERIC(36, 8) NOT NULL,
    -- Deviation percentage from 100.00
    deviation_pct           NUMERIC(10, 6) NOT NULL,
    alert_level             TEXT        NOT NULL DEFAULT 'INVESTIGATION', -- INVESTIGATION | CRITICAL
    resolved                BOOLEAN     NOT NULL DEFAULT false,
    resolved_at             TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_por_discrepancy_alerts_created
    ON por_discrepancy_alerts (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_por_discrepancy_alerts_unresolved
    ON por_discrepancy_alerts (resolved) WHERE resolved = false;
