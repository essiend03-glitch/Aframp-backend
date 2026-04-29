-- Migration: Automated SAR Workflow
-- Tables: sar_reports (state machine), sar_audit_log (immutable)

CREATE TABLE IF NOT EXISTS sar_reports (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    aml_case_id         UUID        NOT NULL,
    transaction_id      UUID        NOT NULL,
    wallet_address      TEXT        NOT NULL,
    status              TEXT        NOT NULL DEFAULT 'Draft'
                            CHECK (status IN ('Draft','PendingReview','Approved','Filed','Acknowledged','Rejected')),
    authority           TEXT        NOT NULL DEFAULT 'NFIU'
                            CHECK (authority IN ('NFIU','CBN')),
    activity_snapshot   JSONB       NOT NULL DEFAULT '{}',
    rendered_report     TEXT,
    reviewed_by         TEXT,
    review_notes        TEXT,
    filed_at            TIMESTAMPTZ,
    acknowledged_at     TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sar_aml_case ON sar_reports (aml_case_id);
CREATE INDEX IF NOT EXISTS idx_sar_status   ON sar_reports (status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_sar_wallet   ON sar_reports (wallet_address, created_at DESC);

-- Immutable audit log — every state transition is recorded here
CREATE TABLE IF NOT EXISTS sar_audit_log (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    sar_id      UUID        NOT NULL REFERENCES sar_reports(id),
    actor_id    TEXT        NOT NULL,
    action      TEXT        NOT NULL,
    from_status TEXT        NOT NULL,
    to_status   TEXT        NOT NULL,
    notes       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sar_audit_sar_id ON sar_audit_log (sar_id, created_at ASC);

-- Immutability trigger: block UPDATE/DELETE on audit log
CREATE OR REPLACE FUNCTION sar_audit_log_immutable()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'sar_audit_log is immutable';
END;
$$;

DROP TRIGGER IF EXISTS trg_sar_audit_immutable ON sar_audit_log;
CREATE TRIGGER trg_sar_audit_immutable
    BEFORE UPDATE OR DELETE ON sar_audit_log
    FOR EACH ROW EXECUTE FUNCTION sar_audit_log_immutable();

-- updated_at trigger for sar_reports
CREATE TRIGGER sar_reports_updated_at
    BEFORE UPDATE ON sar_reports
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

COMMENT ON TABLE sar_reports   IS 'Automated SAR workflow — Draft→PendingReview→Approved→Filed→Acknowledged';
COMMENT ON TABLE sar_audit_log IS 'Immutable audit trail of every SAR state transition';
