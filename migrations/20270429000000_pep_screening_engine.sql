-- PEP Screening & Monitoring Engine — Issue #348

CREATE TABLE pep_matches (
    match_id            UUID PRIMARY KEY,
    consumer_id         UUID NOT NULL,
    matched_name        TEXT NOT NULL,
    matched_aliases     TEXT[] NOT NULL DEFAULT '{}',
    match_score         SMALLINT NOT NULL,
    influence_level     TEXT NOT NULL,
    relationship_type   TEXT NOT NULL,
    jurisdiction        TEXT NOT NULL DEFAULT '',
    cpi_score           SMALLINT NOT NULL DEFAULT 50,
    risk_score          DOUBLE PRECISION NOT NULL,
    risk_tier           TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'pending_review',
    provider_entity_id  TEXT,
    screened_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at         TIMESTAMPTZ,
    reviewed_by         UUID,
    review_notes        TEXT
);

CREATE INDEX idx_pep_matches_consumer ON pep_matches (consumer_id);
CREATE INDEX idx_pep_matches_status   ON pep_matches (status);

CREATE TABLE pep_edd_cases (
    case_id                  UUID PRIMARY KEY,
    consumer_id              UUID NOT NULL,
    match_id                 UUID NOT NULL REFERENCES pep_matches (match_id),
    risk_tier                TEXT NOT NULL,
    status                   TEXT NOT NULL DEFAULT 'open',
    source_of_wealth_notes   TEXT,
    source_of_funds_notes    TEXT,
    assigned_to              UUID,
    requires_senior_signoff  BOOLEAN NOT NULL DEFAULT FALSE,
    senior_signoff_by        UUID,
    senior_signoff_at        TIMESTAMPTZ,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pep_edd_consumer ON pep_edd_cases (consumer_id);
CREATE INDEX idx_pep_edd_status   ON pep_edd_cases (status);

CREATE TABLE pep_audit_log (
    entry_id    UUID PRIMARY KEY,
    consumer_id UUID NOT NULL,
    action      TEXT NOT NULL,
    actor_id    UUID,
    details     JSONB NOT NULL DEFAULT '{}',
    chain_hash  TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pep_audit_consumer ON pep_audit_log (consumer_id, created_at DESC);

-- Add last_pep_screened_at to kyc_records for re-screening scheduling
ALTER TABLE kyc_records
    ADD COLUMN IF NOT EXISTS last_pep_screened_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS full_name             TEXT,
    ADD COLUMN IF NOT EXISTS date_of_birth         DATE,
    ADD COLUMN IF NOT EXISTS nationality           TEXT,
    ADD COLUMN IF NOT EXISTS country_of_residence  TEXT;
