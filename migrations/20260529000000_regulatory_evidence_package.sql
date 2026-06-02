-- Regulatory Examination Support & Evidence Package
-- Stores generated evidence packages, policy version history, and system test reports.

-- ── Evidence packages ─────────────────────────────────────────────────────────
CREATE TABLE regulatory_evidence_packages (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope_label             TEXT NOT NULL,
    period_from             TIMESTAMPTZ NOT NULL,
    period_to               TIMESTAMPTZ NOT NULL,
    generated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    generated_by            TEXT NOT NULL DEFAULT 'system',
    -- SHA-256 of the canonical JSON payload
    checksum_sha256         TEXT NOT NULL,
    -- HMAC-SHA256 signature proving platform origin
    signature_hmac_sha256   TEXT NOT NULL,
    -- Source system counts (summary — full data lives in source tables)
    aml_log_count           BIGINT NOT NULL DEFAULT 0,
    travel_rule_count       BIGINT NOT NULL DEFAULT 0,
    kyc_event_count         BIGINT NOT NULL DEFAULT 0,
    multisig_event_count    BIGINT NOT NULL DEFAULT 0,
    policy_snapshot_count   BIGINT NOT NULL DEFAULT 0,
    system_test_count       BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX idx_reg_evidence_scope    ON regulatory_evidence_packages(scope_label);
CREATE INDEX idx_reg_evidence_period   ON regulatory_evidence_packages(period_from, period_to);
CREATE INDEX idx_reg_evidence_gen_at   ON regulatory_evidence_packages(generated_at DESC);

-- ── Policy version history ────────────────────────────────────────────────────
-- Stores the state of every compliance policy at each point in time.
-- Enables "What was our KYC threshold on January 1st?" queries.
CREATE TABLE regulatory_policy_history (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_name     TEXT NOT NULL,          -- e.g. "kyc_threshold", "aml_ctr_threshold"
    policy_version  TEXT NOT NULL,          -- e.g. "v1.2"
    effective_from  TIMESTAMPTZ NOT NULL,
    effective_until TIMESTAMPTZ,            -- NULL = currently active
    -- Full policy state as JSON (thresholds, rules, limits, etc.)
    policy_state    JSONB NOT NULL,
    changed_by      TEXT NOT NULL,
    change_reason   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_reg_policy_name_time ON regulatory_policy_history(policy_name, effective_from DESC);
CREATE INDEX idx_reg_policy_active    ON regulatory_policy_history(policy_name) WHERE effective_until IS NULL;

-- ── System test & health reports ──────────────────────────────────────────────
-- Stores AML stress tests, pen-test results, DR tests, etc.
-- Attached to evidence packages to prove controls were operating effectively.
CREATE TABLE regulatory_system_test_reports (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    report_type  TEXT NOT NULL,   -- "aml_stress_test" | "pentest" | "security_scan" | "dr_test"
    report_label TEXT NOT NULL,
    executed_at  TIMESTAMPTZ NOT NULL,
    executed_by  TEXT NOT NULL,
    outcome      TEXT NOT NULL CHECK (outcome IN ('pass', 'fail', 'partial')),
    summary      TEXT NOT NULL,
    findings     JSONB NOT NULL DEFAULT '[]',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_reg_test_reports_type ON regulatory_system_test_reports(report_type);
CREATE INDEX idx_reg_test_reports_time ON regulatory_system_test_reports(executed_at DESC);
