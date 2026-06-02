-- PEP Screening & Monitoring Extended Schema
-- Issue #348 - Comprehensive PEP Management

-- Extend pep_matches table with new fields
ALTER TABLE pep_matches 
    ADD COLUMN IF NOT EXISTS pep_profile_id UUID,
    ADD COLUMN IF NOT EXISTS screening_source TEXT DEFAULT 'provider',
    ADD COLUMN IF NOT EXISTS is_family_member BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS is_close_associate BOOLEAN DEFAULT FALSE;

-- Create PEP profiles table (central record for each PEP)
CREATE TABLE pep_profiles (
    pep_profile_id       UUID PRIMARY KEY,
    subject_kyc_id       UUID NOT NULL REFERENCES kyc_records(consumer_id),
    pep_category         TEXT NOT NULL, -- 'domestic_pep', 'foreign_pep', 'international_org_pep'
    pep_position_title   TEXT NOT NULL,
    pep_organization     TEXT,
    pep_country          TEXT NOT NULL,
    pep_status           TEXT NOT NULL DEFAULT 'current', -- 'current', 'former'
    position_start_date  DATE,
    position_end_date    DATE,
    screening_source     TEXT NOT NULL,
    match_confidence_score INTEGER NOT NULL,
    profile_status       TEXT NOT NULL DEFAULT 'under_review', -- 'confirmed', 'under_review', 'false_positive', 'cleared'
    edd_status           TEXT DEFAULT 'not_required',
    assigned_compliance_officer UUID,
    created_timestamp    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_reviewed_timestamp TIMESTAMPTZ
);

CREATE INDEX idx_pep_profiles_kyc ON pep_profiles (subject_kyc_id);
CREATE INDEX idx_pep_profiles_category ON pep_profiles (pep_category);
CREATE INDEX idx_pep_profiles_status ON pep_profiles (profile_status);
CREATE INDEX idx_pep_profiles_edd ON pep_profiles (edd_status);

-- Create PEP family members table
CREATE TABLE pep_family_members (
    family_member_id     UUID PRIMARY KEY,
    pep_profile_id       UUID NOT NULL REFERENCES pep_profiles(pep_profile_id) ON DELETE CASCADE,
    family_member_kyc_id UUID NOT NULL REFERENCES kyc_records(consumer_id),
    relationship_type    TEXT NOT NULL, -- 'spouse', 'child', 'parent', 'sibling'
    screening_status     TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'screened', 'confirmed', 'cleared'
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pep_family_pep ON pep_family_members (pep_profile_id);
CREATE INDEX idx_pep_family_kyc ON pep_family_members (family_member_kyc_id);

-- Create PEP close associates table
CREATE TABLE pep_close_associates (
    associate_id         UUID PRIMARY KEY,
    pep_profile_id       UUID NOT NULL REFERENCES pep_profiles(pep_profile_id) ON DELETE CASCADE,
    associate_kyc_id     UUID NOT NULL REFERENCES kyc_records(consumer_id),
    association_type     TEXT NOT NULL, -- 'business_partner', 'known_associate'
    screening_status     TEXT NOT NULL DEFAULT 'pending',
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pep_associates_pep ON pep_close_associates (pep_profile_id);
CREATE INDEX idx_pep_associates_kyc ON pep_close_associates (associate_kyc_id);

-- Extend EDD cases table with more fields
ALTER TABLE pep_edd_cases
    ADD COLUMN IF NOT EXISTS edd_type TEXT DEFAULT 'standard',
    ADD COLUMN IF NOT EXISTS edd_findings TEXT,
    ADD COLUMN IF NOT EXISTS approval_status TEXT DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS approving_officer UUID,
    ADD COLUMN IF NOT EXISTS completion_timestamp TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS pep_profile_id UUID REFERENCES pep_profiles(pep_profile_id),
    ADD COLUMN IF NOT EXISTS next_renewal_date DATE;

-- Create PEP transaction monitoring table
CREATE TABLE pep_transaction_monitoring (
    monitoring_id        UUID PRIMARY KEY,
    pep_profile_id       UUID NOT NULL REFERENCES pep_profiles(pep_profile_id) ON DELETE CASCADE,
    transaction_id       UUID REFERENCES transactions(transaction_id),
    monitoring_flag      TEXT NOT NULL, -- 'threshold_breach', 'unusual_pattern', 'high_risk_jurisdiction', 'rapid_fund_movement'
    review_status        TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'under_review', 'approved', 'cleared'
    reviewing_officer    UUID,
    review_outcome       TEXT,
    flagged_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at          TIMESTAMPTZ
);

CREATE INDEX idx_pep_txn_profile ON pep_transaction_monitoring (pep_profile_id);
CREATE INDEX idx_pep_txn_status ON pep_transaction_monitoring (review_status);

-- Create PEP database version history
CREATE TABLE pep_database_versions (
    version_id           UUID PRIMARY KEY,
    source_name          TEXT NOT NULL, -- 'dow_jones', 'refinitiv', 'african_pep_db'
    version_hash         TEXT NOT NULL,
    entry_count          INTEGER NOT NULL,
    ingested_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_current           BOOLEAN DEFAULT FALSE
);

CREATE INDEX idx_pep_db_version_source ON pep_database_versions (source_name, ingested_at DESC);

-- Create PEP screening metrics table
CREATE TABLE pep_screening_metrics (
    metric_id            UUID PRIMARY KEY,
    metric_date          DATE NOT NULL,
    total_screened       INTEGER NOT NULL DEFAULT 0,
    pep_detections       INTEGER NOT NULL DEFAULT 0,
    false_positives      INTEGER NOT NULL DEFAULT 0,
    edd_initiated        INTEGER NOT NULL DEFAULT 0,
    edd_completed        INTEGER NOT NULL DEFAULT 0,
    transactions_flagged INTEGER NOT NULL DEFAULT 0,
    transactions_reviewed INTEGER NOT NULL DEFAULT 0,
    avg_detection_to_edd_completion_days REAL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pep_metrics_date ON pep_screening_metrics (metric_date DESC);

-- Add monitoring status fields to pep_profiles
ALTER TABLE pep_profiles 
    ADD COLUMN IF NOT EXISTS monitoring_start_date TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS monitoring_end_date TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS is_under_winddown BOOLEAN DEFAULT FALSE;

-- Add PEP database status table
CREATE TABLE pep_database_status (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_name          TEXT NOT NULL UNIQUE,
    last_update          TIMESTAMPTZ,
    total_entries        INTEGER DEFAULT 0,
    index_health         TEXT DEFAULT 'healthy',
    config               JSONB DEFAULT '{}'
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_pep_profiles_country ON pep_profiles (pep_country);
CREATE INDEX IF NOT EXISTS idx_pep_profiles_former ON pep_profiles (pep_status, monitoring_end_date);