-- Issue #475: Partner Compliance & Due Diligence Framework
-- Creates partner_compliance_profiles, partner_kyb_documents,
-- partner_due_diligence_checks, and partner_compliance_audit_logs tables.

CREATE TABLE IF NOT EXISTS partner_compliance_profiles (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id                  UUID NOT NULL UNIQUE,
    corporate_registration_code TEXT NOT NULL,
    tax_identifier              TEXT,
    ubo_structure               JSONB NOT NULL DEFAULT '[]',
    aggregate_risk_rating       TEXT NOT NULL DEFAULT 'medium'
                                    CHECK (aggregate_risk_rating IN ('low','medium','high','critical')),
    risk_score                  DOUBLE PRECISION NOT NULL DEFAULT 0,
    status                      TEXT NOT NULL DEFAULT 'pending'
                                    CHECK (status IN ('pending','verified','suspended','rejected')),
    tier_limit_config           JSONB NOT NULL DEFAULT '{}',
    due_diligence_expires_at    TIMESTAMPTZ,
    last_reviewed_at            TIMESTAMPTZ,
    reviewed_by                 TEXT,
    review_notes                TEXT,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS partner_kyb_documents (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id           UUID NOT NULL REFERENCES partner_compliance_profiles(partner_id) ON DELETE CASCADE,
    document_type        TEXT NOT NULL,
    file_name            TEXT NOT NULL,
    file_sha256          TEXT NOT NULL,   -- SHA-256 hex; raw bytes stored in object storage
    storage_path         TEXT NOT NULL,
    verification_status  TEXT NOT NULL DEFAULT 'pending'
                             CHECK (verification_status IN ('pending','approved','rejected','expired')),
    expires_at           TIMESTAMPTZ,
    staff_notes          TEXT,
    uploaded_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    verified_at          TIMESTAMPTZ,
    verified_by          TEXT
);

CREATE TABLE IF NOT EXISTS partner_due_diligence_checks (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id   UUID NOT NULL REFERENCES partner_compliance_profiles(partner_id) ON DELETE CASCADE,
    check_type   TEXT NOT NULL,   -- 'sanctions' | 'pep' | 'watchlist' | 'registry'
    provider     TEXT NOT NULL,
    result       TEXT NOT NULL CHECK (result IN ('clear','hit','error')),
    hit_details  JSONB,
    checked_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS partner_compliance_audit_logs (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id    UUID NOT NULL,
    analyst_id    TEXT NOT NULL,
    action        TEXT NOT NULL,
    justification TEXT NOT NULL,
    metadata      JSONB NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_pcp_status ON partner_compliance_profiles(status);
CREATE INDEX IF NOT EXISTS idx_pkd_partner_status ON partner_kyb_documents(partner_id, verification_status);
CREATE INDEX IF NOT EXISTS idx_pddc_partner_type ON partner_due_diligence_checks(partner_id, check_type);
CREATE INDEX IF NOT EXISTS idx_pcal_partner ON partner_compliance_audit_logs(partner_id, created_at DESC);
