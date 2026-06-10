-- migrate:up
-- Reconciliation Worker: discrepancy_log table and supporting indexes.
-- Tracks three-way mismatches between bank deposits, mint_requests, and on-chain events.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'discrepancy_type') THEN
        CREATE TYPE discrepancy_type AS ENUM (
            'MISSING_MINT',
            'UNAUTHORIZED_MINT',
            'AMOUNT_MISMATCH'
        );
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'discrepancy_status') THEN
        CREATE TYPE discrepancy_status AS ENUM (
            'OPEN',
            'INVESTIGATING',
            'RESOLVED'
        );
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS discrepancy_log (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id      UUID REFERENCES transactions(transaction_id) ON DELETE SET NULL,
    discrepancy_type    discrepancy_type NOT NULL,
    status              discrepancy_status NOT NULL DEFAULT 'OPEN',

    -- Three-way match evidence
    fiat_amount         NUMERIC(36, 18),
    mint_amount         NUMERIC(36, 18),
    stellar_tx_hash     TEXT,
    payment_reference   TEXT,

    -- Audit fields
    detected_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at         TIMESTAMPTZ,
    resolved_by         TEXT,
    notes               TEXT,

    -- Alert tracking
    alert_sent          BOOLEAN NOT NULL DEFAULT FALSE,
    alert_sent_at       TIMESTAMPTZ,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

DROP TRIGGER IF EXISTS set_updated_at_discrepancy_log ON discrepancy_log;
CREATE TRIGGER set_updated_at_discrepancy_log
    BEFORE UPDATE ON discrepancy_log
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Reconciliation health reports (end-of-day summaries)
ALTER TABLE reconciliation_reports
    ADD COLUMN IF NOT EXISTS report_date DATE,
    ADD COLUMN IF NOT EXISTS total_transactions INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS matched_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS discrepancy_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS missing_mint_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS unauthorized_mint_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS amount_mismatch_count INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS has_open_discrepancies BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS period_closed BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS generated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE UNIQUE INDEX IF NOT EXISTS idx_reconciliation_reports_report_date
    ON reconciliation_reports (report_date)
    WHERE report_date IS NOT NULL;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_discrepancy_log_status ON discrepancy_log (status) WHERE status != 'RESOLVED';
CREATE INDEX IF NOT EXISTS idx_discrepancy_log_type   ON discrepancy_log (discrepancy_type);
CREATE INDEX IF NOT EXISTS idx_discrepancy_log_tx     ON discrepancy_log (transaction_id);
CREATE INDEX IF NOT EXISTS idx_discrepancy_log_detected ON discrepancy_log (detected_at);

-- migrate:down
DROP INDEX IF EXISTS idx_discrepancy_log_detected;
DROP INDEX IF EXISTS idx_discrepancy_log_tx;
DROP INDEX IF EXISTS idx_discrepancy_log_type;
DROP INDEX IF EXISTS idx_discrepancy_log_status;
DROP TABLE IF EXISTS reconciliation_reports;
DROP TABLE IF EXISTS discrepancy_log;
DROP TYPE IF EXISTS discrepancy_status;
DROP TYPE IF EXISTS discrepancy_type;
