-- Migration: Sanctions Screening Engine — Issue #419
-- Creates two tables:
--   1. sanctions_screening_log  — immutable audit trail of every screening call
--   2. bypass_audit             — dual-authorisation bypass records
--
-- Immutability is enforced by a trigger that prevents UPDATE/DELETE on
-- sanctions_screening_log.

-- ── 1. Screening log ─────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS sanctions_screening_log (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id  UUID        NOT NULL,
    -- 'Clear' | 'Hit' | 'ProviderError'
    outcome         TEXT        NOT NULL,
    -- JSON array of SanctionsMatch objects
    matches_json    JSONB       NOT NULL DEFAULT '[]',
    latency_ms      BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Fast lookup by transaction
CREATE INDEX IF NOT EXISTS idx_ssl_transaction_id
    ON sanctions_screening_log (transaction_id, created_at DESC);

-- Partial index for quick "any hits?" queries
CREATE INDEX IF NOT EXISTS idx_ssl_hits
    ON sanctions_screening_log (transaction_id)
    WHERE outcome = 'Hit';

-- Immutability trigger: block UPDATE and DELETE
CREATE OR REPLACE FUNCTION sanctions_log_immutable()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'sanctions_screening_log is immutable — rows may not be modified or deleted';
END;
$$;

DROP TRIGGER IF EXISTS trg_ssl_immutable ON sanctions_screening_log;
CREATE TRIGGER trg_ssl_immutable
    BEFORE UPDATE OR DELETE ON sanctions_screening_log
    FOR EACH ROW EXECUTE FUNCTION sanctions_log_immutable();

-- ── 2. Bypass audit ───────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS bypass_audit (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id      UUID        NOT NULL,
    reason              TEXT        NOT NULL,
    first_approver_id   TEXT        NOT NULL,
    second_approver_id  TEXT,
    approved            BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    approved_at         TIMESTAMPTZ,

    -- Dual-auth constraint: second approver must differ from first
    CONSTRAINT bypass_dual_auth
        CHECK (second_approver_id IS NULL OR second_approver_id <> first_approver_id)
);

CREATE INDEX IF NOT EXISTS idx_bypass_transaction_id
    ON bypass_audit (transaction_id);

CREATE INDEX IF NOT EXISTS idx_bypass_approved
    ON bypass_audit (transaction_id)
    WHERE approved = TRUE;

COMMENT ON TABLE sanctions_screening_log IS
    'Immutable audit trail of every real-time sanctions screening call (Issue #419).';

COMMENT ON TABLE bypass_audit IS
    'Dual-authorisation bypass records for compliance overrides (Issue #419).';
