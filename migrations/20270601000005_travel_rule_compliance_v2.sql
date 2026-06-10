-- Migration: Travel Rule Compliance v2 (Issue #452)
-- FATF Recommendation 16 — full VASP-to-VASP PII exchange
--
-- Maintainer action required before merge/QA:
--   1. sqlx migrate run  (or project equivalent)
--   2. Confirm enum alterations on DBs already carrying 20270428000002
--   3. Run: cargo test --test travel_rule_integration  (with DATABASE_URL set)
-- Rollback: drop the new columns/tables and run ALTER TYPE ... DROP VALUE (PG 14+)

-- ---------------------------------------------------------------------------
-- Extend the existing travel_rule_protocol enum
-- ADD VALUE IF NOT EXISTS is idempotent on PostgreSQL 13+
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    ALTER TYPE travel_rule_protocol ADD VALUE IF NOT EXISTS 'trp';
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- ---------------------------------------------------------------------------
-- Add travel_rule_direction enum
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    CREATE TYPE travel_rule_direction AS ENUM ('outbound', 'inbound');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- ---------------------------------------------------------------------------
-- Add vasp_trust_status enum
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    CREATE TYPE vasp_trust_status AS ENUM ('verified', 'unverified', 'blocked');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- ---------------------------------------------------------------------------
-- Add vasp_regulatory_status enum
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    CREATE TYPE vasp_regulatory_status AS ENUM ('licensed', 'unlicensed', 'pending', 'suspended');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- ---------------------------------------------------------------------------
-- Extend vasp_registry
-- ---------------------------------------------------------------------------
ALTER TABLE vasp_registry
    ADD COLUMN IF NOT EXISTS vasp_did          TEXT,
    ADD COLUMN IF NOT EXISTS lei               TEXT,
    ADD COLUMN IF NOT EXISTS regulatory_status vasp_regulatory_status NOT NULL DEFAULT 'unlicensed',
    ADD COLUMN IF NOT EXISTS trust_status      vasp_trust_status      NOT NULL DEFAULT 'unverified',
    ADD COLUMN IF NOT EXISTS public_key_pem    TEXT,
    ADD COLUMN IF NOT EXISTS interaction_count INT                    NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_interaction_at TIMESTAMPTZ;

-- Known wallet address prefixes for automatic VASP discovery
CREATE TABLE IF NOT EXISTS vasp_wallet_labels (
    id           UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    vasp_id      TEXT         NOT NULL REFERENCES vasp_registry(vasp_id) ON DELETE CASCADE,
    address_prefix TEXT       NOT NULL,
    network      TEXT         NOT NULL DEFAULT 'stellar',
    created_at   TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_vasp_wallet_labels_prefix
    ON vasp_wallet_labels (address_prefix);

-- ---------------------------------------------------------------------------
-- Extend travel_rule_exchanges
-- ---------------------------------------------------------------------------
ALTER TABLE travel_rule_exchanges
    ADD COLUMN IF NOT EXISTS direction             travel_rule_direction NOT NULL DEFAULT 'outbound',
    ADD COLUMN IF NOT EXISTS sunrise_rule_applied  BOOLEAN              NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS sla_window_secs       INT                  NOT NULL DEFAULT 300,
    ADD COLUMN IF NOT EXISTS sla_breached          BOOLEAN              NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS sla_breached_at       TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS screening_result      JSONB,
    ADD COLUMN IF NOT EXISTS compliance_case_id    UUID,
    ADD COLUMN IF NOT EXISTS retry_count           INT                  NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_retry_at         TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS pending_travel_rule   BOOLEAN              NOT NULL DEFAULT TRUE;

CREATE INDEX IF NOT EXISTS idx_travel_rule_direction
    ON travel_rule_exchanges (direction);
CREATE INDEX IF NOT EXISTS idx_travel_rule_sla_breach
    ON travel_rule_exchanges (sla_breached, timeout_at)
    WHERE status = 'pending';

-- ---------------------------------------------------------------------------
-- Travel Rule thresholds (per currency, transaction type, jurisdiction)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS travel_rule_thresholds (
    id               UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    currency         TEXT         NOT NULL,
    transaction_type TEXT         NOT NULL,   -- 'cngn_transfer' | 'offramp' | 'cross_border'
    jurisdiction     TEXT         NOT NULL DEFAULT 'NG',
    threshold_amount NUMERIC(24, 8) NOT NULL,
    is_active        BOOLEAN      NOT NULL DEFAULT TRUE,
    approved_by      UUID,
    approved_at      TIMESTAMPTZ,
    created_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_travel_rule_threshold
        UNIQUE (currency, transaction_type, jurisdiction)
);

-- Default threshold: cNGN transfers ≥ 500,000 NGN (≈ $500 USD equivalent)
INSERT INTO travel_rule_thresholds (currency, transaction_type, jurisdiction, threshold_amount)
VALUES
    ('cNGN', 'cngn_transfer',  'NG', 500000),
    ('cNGN', 'offramp',        'NG', 500000),
    ('cNGN', 'cross_border',   'NG', 0)        -- all cross-border, per FATF
ON CONFLICT (currency, transaction_type, jurisdiction) DO NOTHING;

-- ---------------------------------------------------------------------------
-- Unhosted wallet policy
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS travel_rule_unhosted_wallet_policy (
    id                              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_type                     TEXT        NOT NULL DEFAULT 'allow_below_threshold',
        -- 'allow' | 'allow_below_threshold' | 'require_attestation' | 'block'
    threshold_amount                NUMERIC(24, 8),
    threshold_currency              TEXT,
    updated_by_compliance_officer   UUID,
    updated_by_senior_management    UUID,
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed default policy
INSERT INTO travel_rule_unhosted_wallet_policy
    (policy_type, threshold_amount, threshold_currency)
VALUES
    ('allow_below_threshold', 500000, 'cNGN')
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- User self-attestation for unhosted wallet transactions
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS travel_rule_attestations (
    id               UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          UUID         NOT NULL,
    transaction_id   TEXT         NOT NULL,
    wallet_address   TEXT         NOT NULL,
    attested_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    ip_address       TEXT,
    user_agent       TEXT
);

CREATE INDEX IF NOT EXISTS idx_travel_rule_attestations_tx
    ON travel_rule_attestations (transaction_id);
CREATE INDEX IF NOT EXISTS idx_travel_rule_attestations_user
    ON travel_rule_attestations (user_id);
