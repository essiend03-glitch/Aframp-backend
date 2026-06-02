-- Partner Revenue Sharing & Commission Management Engine (Issue #471)
-- Double-entry ledger, commission structures, and payout records.
-- All monetary amounts are stored in stroops (1 cNGN = 10_000_000 stroops).

-- ---------------------------------------------------------------------------
-- Enums
-- ---------------------------------------------------------------------------
DO $$ BEGIN
    CREATE TYPE commission_type AS ENUM ('percentage', 'fixed_fiat', 'tiered');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    CREATE TYPE ledger_direction AS ENUM ('credit', 'debit');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    CREATE TYPE payout_status AS ENUM ('pending', 'processing', 'completed', 'failed', 'cancelled');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- ---------------------------------------------------------------------------
-- commission_structures — contractual fee-split agreements per partner
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS commission_structures (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id          UUID NOT NULL REFERENCES partners(id) ON DELETE RESTRICT,
    name                TEXT NOT NULL,
    commission_type     commission_type NOT NULL,

    -- For 'percentage': share of gross fee (e.g. 0.35 = 35%)
    percentage_rate     NUMERIC(8, 7),           -- up to 9.9999999 (7 dp, Stellar precision)

    -- For 'fixed_fiat': fixed cNGN amount per transaction in stroops
    fixed_stroops       BIGINT,

    -- For 'tiered': JSON array of {min_volume_stroops, max_volume_stroops, rate}
    tiers               JSONB,

    -- Volume thresholds (stroops) for tier activation
    min_volume_stroops  BIGINT NOT NULL DEFAULT 0,
    max_volume_stroops  BIGINT,                  -- NULL = unlimited

    corridor            TEXT,                    -- NULL = all corridors
    is_active           BOOLEAN NOT NULL DEFAULT TRUE,
    effective_from      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    effective_to        TIMESTAMPTZ,
    created_by          UUID NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT chk_percentage CHECK (
        commission_type != 'percentage' OR (percentage_rate IS NOT NULL AND percentage_rate >= 0 AND percentage_rate <= 1)
    ),
    CONSTRAINT chk_fixed CHECK (
        commission_type != 'fixed_fiat' OR (fixed_stroops IS NOT NULL AND fixed_stroops >= 0)
    ),
    CONSTRAINT chk_tiered CHECK (
        commission_type != 'tiered' OR tiers IS NOT NULL
    )
);

CREATE INDEX IF NOT EXISTS idx_commission_structures_partner
    ON commission_structures (partner_id, is_active, effective_from);
CREATE INDEX IF NOT EXISTS idx_commission_structures_corridor
    ON commission_structures (corridor) WHERE corridor IS NOT NULL;

-- ---------------------------------------------------------------------------
-- partner_revenue_ledger — strict double-entry ledger for partner commissions
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS partner_revenue_ledger (
    entry_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id          UUID NOT NULL REFERENCES partners(id) ON DELETE RESTRICT,
    transaction_id      UUID NOT NULL,           -- source payment/onramp transaction
    commission_structure_id UUID REFERENCES commission_structures(id),

    amount_stroops      BIGINT NOT NULL CHECK (amount_stroops > 0),
    direction           ledger_direction NOT NULL,

    -- Running balance after this entry (maintained by the service layer)
    balance_after_stroops BIGINT NOT NULL,

    gross_fee_stroops   BIGINT NOT NULL,         -- gross fee collected on source tx
    platform_share_stroops BIGINT NOT NULL,      -- platform's portion
    tier_index          SMALLINT,                -- which tier was applied, NULL if not tiered
    corridor            TEXT,
    narrative           TEXT NOT NULL,           -- human-readable reason

    -- On-chain evidence
    stellar_tx_hash     TEXT,                    -- populated after payout
    payout_record_id    UUID,                    -- FK added below after payout table

    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Invariant: gross_fee = platform_share + partner_commission (enforced in service)
    CONSTRAINT chk_invariant CHECK (
        gross_fee_stroops = platform_share_stroops + amount_stroops
    )
);

-- Append-only: no UPDATE or DELETE allowed (enforced via trigger)
CREATE OR REPLACE FUNCTION partner_ledger_immutable()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'partner_revenue_ledger entries are immutable';
END;
$$;

DROP TRIGGER IF EXISTS trg_partner_ledger_immutable ON partner_revenue_ledger;
CREATE TRIGGER trg_partner_ledger_immutable
    BEFORE UPDATE OR DELETE ON partner_revenue_ledger
    FOR EACH ROW EXECUTE FUNCTION partner_ledger_immutable();

CREATE INDEX IF NOT EXISTS idx_prl_partner_created
    ON partner_revenue_ledger (partner_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_prl_transaction
    ON partner_revenue_ledger (transaction_id);
CREATE INDEX IF NOT EXISTS idx_prl_payout
    ON partner_revenue_ledger (payout_record_id) WHERE payout_record_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- commission_payout_records — batch settlement records to partner wallets
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS commission_payout_records (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id          UUID NOT NULL REFERENCES partners(id) ON DELETE RESTRICT,
    payout_address      TEXT NOT NULL,           -- partner's Stellar wallet address
    total_stroops       BIGINT NOT NULL CHECK (total_stroops > 0),
    entry_count         INT NOT NULL DEFAULT 0,  -- number of ledger entries included
    status              payout_status NOT NULL DEFAULT 'pending',
    stellar_tx_hash     TEXT UNIQUE,             -- immutable once set
    batch_ref           TEXT NOT NULL,           -- e.g. "2026-W22" for traceability
    initiated_by        UUID NOT NULL,           -- admin user or system UUID
    error_message       TEXT,
    attempted_at        TIMESTAMPTZ,
    completed_at        TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cpr_partner_status
    ON commission_payout_records (partner_id, status);
CREATE INDEX IF NOT EXISTS idx_cpr_status
    ON commission_payout_records (status) WHERE status IN ('pending', 'processing');

-- ---------------------------------------------------------------------------
-- FK: ledger → payout_record (deferred to avoid circular dep at create time)
-- ---------------------------------------------------------------------------
ALTER TABLE partner_revenue_ledger
    ADD CONSTRAINT fk_prl_payout_record
    FOREIGN KEY (payout_record_id) REFERENCES commission_payout_records(id)
    DEFERRABLE INITIALLY DEFERRED;

-- ---------------------------------------------------------------------------
-- partner_commission_balances — materialised running balance per partner
-- (updated atomically with ledger writes; used for fast balance reads)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS partner_commission_balances (
    partner_id          UUID PRIMARY KEY REFERENCES partners(id) ON DELETE RESTRICT,
    accrued_stroops     BIGINT NOT NULL DEFAULT 0 CHECK (accrued_stroops >= 0),
    paid_stroops        BIGINT NOT NULL DEFAULT 0 CHECK (paid_stroops >= 0),
    last_entry_id       UUID REFERENCES partner_revenue_ledger(entry_id),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE commission_structures IS 'Partner fee-split contractual agreements with tiered volume support';
COMMENT ON TABLE partner_revenue_ledger IS 'Append-only double-entry ledger for partner commission accrual';
COMMENT ON TABLE commission_payout_records IS 'Batch settlement records linking ledger to on-chain Stellar transactions';
COMMENT ON TABLE partner_commission_balances IS 'Materialised running balance for O(1) balance reads';
