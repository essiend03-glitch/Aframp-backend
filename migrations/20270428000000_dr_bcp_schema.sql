-- Migration: DR/BCP Schema (Issue #DR-BCP)
-- Disaster Recovery & Business Continuity Planning tables.

-- ---------------------------------------------------------------------------
-- Custom types
-- ---------------------------------------------------------------------------

CREATE TYPE service_criticality AS ENUM ('critical', 'high', 'low');
CREATE TYPE dr_incident_status  AS ENUM ('declared', 'active', 'recovering', 'resolved', 'post_mortem_pending');
CREATE TYPE restore_test_result AS ENUM ('passed', 'failed', 'partial');
CREATE TYPE regulatory_body     AS ENUM ('cbn', 'sec', 'partner_fi', 'internal');

-- ---------------------------------------------------------------------------
-- Business Impact Analysis
-- ---------------------------------------------------------------------------

CREATE TABLE dr_bia_entries (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_name    TEXT NOT NULL UNIQUE,
    criticality     service_criticality NOT NULL,
    -- Maximum Tolerable Downtime in seconds
    mtd_seconds     BIGINT NOT NULL,
    -- Recovery Point Objective target in seconds (0 = zero data loss)
    rpo_seconds     BIGINT NOT NULL DEFAULT 0,
    -- Recovery Time Objective target in seconds (900 = 15 min)
    rto_seconds     BIGINT NOT NULL DEFAULT 900,
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed BIA with Aframp's critical services
INSERT INTO dr_bia_entries (service_name, criticality, mtd_seconds, rpo_seconds, rto_seconds, description) VALUES
    ('stellar-settlement',   'critical', 300,   0,   900, 'Stellar payment settlement — zero tolerance for data loss'),
    ('payment-processing',   'critical', 300,   0,   900, 'On-ramp / off-ramp payment orchestration'),
    ('kyc-compliance',       'high',     3600,  0,   900, 'KYC verification and compliance monitoring'),
    ('wallet-service',       'high',     3600,  0,   900, 'User wallet balances and transfers'),
    ('exchange-rates',       'high',     1800,  300, 900, 'Exchange rate feed — 5 min staleness tolerated'),
    ('analytics-reporting',  'low',      86400, 3600, 3600, 'Analytics dashboards and partner reports');

-- ---------------------------------------------------------------------------
-- Immutable backup records
-- ---------------------------------------------------------------------------

CREATE TABLE dr_backup_records (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    s3_key           TEXT NOT NULL,
    s3_bucket        TEXT NOT NULL,
    checksum_sha256  TEXT NOT NULL,
    size_bytes       BIGINT NOT NULL,
    verified         BOOLEAN NOT NULL DEFAULT FALSE,
    last_verified_at TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dr_backup_records_created ON dr_backup_records (created_at DESC);

-- ---------------------------------------------------------------------------
-- Restore test runs
-- ---------------------------------------------------------------------------

CREATE TABLE dr_restore_test_runs (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    backup_id                 UUID NOT NULL REFERENCES dr_backup_records(id),
    result                    restore_test_result NOT NULL,
    restore_duration_seconds  BIGINT NOT NULL,
    rpo_achieved_seconds      BIGINT,
    rto_achieved_seconds      BIGINT,
    error_message             TEXT,
    run_at                    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dr_restore_runs_backup ON dr_restore_test_runs (backup_id);
CREATE INDEX idx_dr_restore_runs_at     ON dr_restore_test_runs (run_at DESC);

-- ---------------------------------------------------------------------------
-- DR incidents
-- ---------------------------------------------------------------------------

CREATE TABLE dr_incidents (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title                 TEXT NOT NULL,
    description           TEXT NOT NULL,
    status                dr_incident_status NOT NULL DEFAULT 'declared',
    commander_id          TEXT NOT NULL,
    affected_services     JSONB NOT NULL DEFAULT '[]',
    rpo_achieved_seconds  BIGINT,
    rto_achieved_seconds  BIGINT,
    declared_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at           TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dr_incidents_status ON dr_incidents (status);

-- ---------------------------------------------------------------------------
-- Regulatory notifications
-- ---------------------------------------------------------------------------

CREATE TABLE dr_regulatory_notifications (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id     UUID NOT NULL REFERENCES dr_incidents(id),
    body            regulatory_body NOT NULL,
    template_used   TEXT NOT NULL,
    sent_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at TIMESTAMPTZ
);

CREATE INDEX idx_dr_reg_notif_incident ON dr_regulatory_notifications (incident_id);
