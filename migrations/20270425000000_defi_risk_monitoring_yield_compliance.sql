-- Migration: DeFi Risk Assessment, Position Monitoring, Yield Distribution & Regulatory Compliance
-- Tasks 1–4 of Domain 6 — DeFi & Yield Integration

-- ── Enums ─────────────────────────────────────────────────────────────────────

CREATE TYPE protocol_risk_tier AS ENUM ('green', 'amber', 'red');
CREATE TYPE audit_remediation_status AS ENUM ('fully_remediated', 'partially_remediated', 'unresolved', 'acknowledged');
CREATE TYPE vulnerability_severity AS ENUM ('critical', 'high', 'medium', 'low', 'informational');
CREATE TYPE governance_proposal_type AS ENUM ('parameter_change', 'fee_change', 'asset_listing', 'asset_delisting', 'upgrade_contract', 'emergency_action', 'other');
CREATE TYPE proposal_outcome AS ENUM ('passed', 'rejected', 'cancelled', 'pending');
CREATE TYPE risk_report_type AS ENUM ('weekly_governance', 'monthly_management', 'ad_hoc');
CREATE TYPE monitoring_alert_level AS ENUM ('informational', 'warning', 'critical', 'emergency');
CREATE TYPE yield_source_type AS ENUM ('amm_trading_fees', 'lending_interest', 'liquidity_mining_incentives', 'platform_treasury_contribution');
CREATE TYPE defi_regulatory_category AS ENUM ('asset_management', 'lending', 'exchange', 'custody');
CREATE TYPE defi_operation_type AS ENUM ('deposit', 'withdrawal', 'borrow', 'repay', 'liquidity_provision', 'liquidity_removal', 'swap', 'yield_claim');
CREATE TYPE defi_report_type AS ENUM ('nigerian_sec_digital_asset', 'nfiu_defi_activity', 'monthly_aggregate_activity', 'quarterly_risk_summary', 'annual_compliance_summary', 'ad_hoc');
CREATE TYPE report_filing_status AS ENUM ('draft', 'review', 'approved', 'filed', 'acknowledged');
CREATE TYPE regulatory_change_status AS ENUM ('identified', 'in_progress', 'implemented', 'deferred');

-- ── Task 1: Risk Assessment ───────────────────────────────────────────────────

CREATE TABLE defi_protocol_audits (
    audit_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id         TEXT NOT NULL,
    audit_firm          TEXT NOT NULL,
    audit_date          TIMESTAMPTZ NOT NULL,
    audit_scope         TEXT NOT NULL,
    critical_findings   INT NOT NULL DEFAULT 0,
    high_findings       INT NOT NULL DEFAULT 0,
    medium_findings     INT NOT NULL DEFAULT 0,
    low_findings        INT NOT NULL DEFAULT 0,
    unresolved_critical INT NOT NULL DEFAULT 0,
    remediation_status  audit_remediation_status NOT NULL DEFAULT 'unresolved',
    report_url          TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_protocol_audits_protocol ON defi_protocol_audits(protocol_id, audit_date DESC);

CREATE TABLE defi_vulnerability_disclosures (
    disclosure_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id                 TEXT NOT NULL,
    severity                    vulnerability_severity NOT NULL,
    title                       TEXT NOT NULL,
    description                 TEXT NOT NULL,
    cve_id                      TEXT,
    disclosed_at                TIMESTAMPTZ NOT NULL,
    patched_at                  TIMESTAMPTZ,
    affects_platform_positions  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_vuln_protocol ON defi_vulnerability_disclosures(protocol_id, disclosed_at DESC);

CREATE TABLE defi_unplanned_upgrades (
    upgrade_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id             TEXT NOT NULL,
    detected_at             TIMESTAMPTZ NOT NULL,
    previous_implementation TEXT NOT NULL,
    new_implementation      TEXT NOT NULL,
    was_announced           BOOLEAN NOT NULL DEFAULT FALSE,
    announcement_url        TEXT,
    risk_assessment         TEXT NOT NULL DEFAULT '',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE defi_economic_metrics (
    metric_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id                 TEXT NOT NULL,
    tvl                         NUMERIC(30,8) NOT NULL DEFAULT 0,
    tvl_change_1h_pct           DOUBLE PRECISION NOT NULL DEFAULT 0,
    tvl_change_24h_pct          DOUBLE PRECISION NOT NULL DEFAULT 0,
    utilisation_rate            DOUBLE PRECISION NOT NULL DEFAULT 0,
    oracle_price_deviation_pct  DOUBLE PRECISION NOT NULL DEFAULT 0,
    oracle_last_updated_at      TIMESTAMPTZ,
    oracle_is_stale             BOOLEAN NOT NULL DEFAULT FALSE,
    liquidation_rate_24h        DOUBLE PRECISION NOT NULL DEFAULT 0,
    volume_24h                  NUMERIC(30,8) NOT NULL DEFAULT 0,
    recorded_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_economic_metrics_protocol ON defi_economic_metrics(protocol_id, recorded_at DESC);

CREATE TABLE defi_governance_proposals (
    proposal_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id                 TEXT NOT NULL,
    title                       TEXT NOT NULL,
    description                 TEXT NOT NULL,
    proposal_type               governance_proposal_type NOT NULL,
    voting_start                TIMESTAMPTZ NOT NULL,
    voting_end                  TIMESTAMPTZ NOT NULL,
    votes_for                   NUMERIC(30,8) NOT NULL DEFAULT 0,
    votes_against               NUMERIC(30,8) NOT NULL DEFAULT 0,
    quorum_reached              BOOLEAN NOT NULL DEFAULT FALSE,
    outcome                     proposal_outcome,
    material_impact_on_platform BOOLEAN NOT NULL DEFAULT FALSE,
    impact_assessment           TEXT,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_governance_protocol ON defi_governance_proposals(protocol_id, voting_start DESC);

CREATE TABLE defi_composite_risk_scores (
    score_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id         TEXT NOT NULL,
    composite_score     DOUBLE PRECISION NOT NULL,
    smart_contract_score DOUBLE PRECISION NOT NULL,
    economic_score      DOUBLE PRECISION NOT NULL,
    operational_score   DOUBLE PRECISION NOT NULL,
    concentration_score DOUBLE PRECISION NOT NULL,
    risk_tier           protocol_risk_tier NOT NULL,
    computed_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_risk_scores_protocol ON defi_composite_risk_scores(protocol_id, computed_at DESC);

CREATE TABLE defi_risk_score_history (
    history_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id     TEXT NOT NULL,
    composite_score DOUBLE PRECISION NOT NULL,
    risk_tier       protocol_risk_tier NOT NULL,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_risk_history_protocol ON defi_risk_score_history(protocol_id, recorded_at DESC);

CREATE TABLE defi_stress_test_results (
    result_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_id         UUID NOT NULL,
    scenario_name       TEXT NOT NULL,
    estimated_loss      NUMERIC(30,8) NOT NULL DEFAULT 0,
    estimated_loss_pct  DOUBLE PRECISION NOT NULL DEFAULT 0,
    affected_protocols  TEXT[] NOT NULL DEFAULT '{}',
    affected_positions  UUID[] NOT NULL DEFAULT '{}',
    run_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    triggered_by        TEXT NOT NULL
);

CREATE TABLE defi_risk_reports (
    report_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    report_type     risk_report_type NOT NULL,
    period_start    TIMESTAMPTZ NOT NULL,
    period_end      TIMESTAMPTZ NOT NULL,
    generated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    generated_by    TEXT NOT NULL,
    download_url    TEXT,
    summary         JSONB NOT NULL DEFAULT '{}'
);

-- ── Task 2: Position Monitoring ───────────────────────────────────────────────

CREATE TABLE defi_position_snapshots (
    snapshot_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    position_id             UUID NOT NULL,
    protocol_id             TEXT NOT NULL,
    strategy_id             UUID,
    current_value           NUMERIC(30,8) NOT NULL DEFAULT 0,
    current_value_fiat      NUMERIC(30,8) NOT NULL DEFAULT 0,
    value_change_abs        NUMERIC(30,8) NOT NULL DEFAULT 0,
    value_change_pct        DOUBLE PRECISION NOT NULL DEFAULT 0,
    cumulative_change_pct   DOUBLE PRECISION NOT NULL DEFAULT 0,
    allocation_drift_pct    DOUBLE PRECISION NOT NULL DEFAULT 0,
    impermanent_loss_pct    DOUBLE PRECISION NOT NULL DEFAULT 0,
    accrued_fees            NUMERIC(30,8) NOT NULL DEFAULT 0,
    protocol_health_score   DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    alert_level             monitoring_alert_level NOT NULL DEFAULT 'informational',
    snapshotted_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_snapshots_position ON defi_position_snapshots(position_id, snapshotted_at DESC);

CREATE TABLE defi_monitoring_alerts (
    alert_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_level         monitoring_alert_level NOT NULL,
    position_id         UUID,
    protocol_id         TEXT,
    strategy_id         UUID,
    message             TEXT NOT NULL,
    recommended_action  TEXT NOT NULL DEFAULT '',
    acknowledged_by     TEXT,
    acknowledged_at     TIMESTAMPTZ,
    resolved_by         TEXT,
    resolved_at         TIMESTAMPTZ,
    resolution_notes    TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_alerts_unresolved ON defi_monitoring_alerts(resolved_at) WHERE resolved_at IS NULL;

CREATE TABLE defi_protocol_health_history (
    history_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id     TEXT NOT NULL,
    health_score    DOUBLE PRECISION NOT NULL,
    tvl             NUMERIC(30,8) NOT NULL DEFAULT 0,
    volume_24h      NUMERIC(30,8) NOT NULL DEFAULT 0,
    active_users    BIGINT NOT NULL DEFAULT 0,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_health_history_protocol ON defi_protocol_health_history(protocol_id, recorded_at DESC);

CREATE TABLE defi_impermanent_loss_records (
    record_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    position_id         UUID NOT NULL,
    pool_id             TEXT NOT NULL,
    current_il_pct      DOUBLE PRECISION NOT NULL DEFAULT 0,
    cumulative_il_pct   DOUBLE PRECISION NOT NULL DEFAULT 0,
    deposit_price_ratio DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    current_price_ratio DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    computed_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_il_position ON defi_impermanent_loss_records(position_id, computed_at DESC);

CREATE TABLE defi_rebalancing_audit (
    event_id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id             UUID NOT NULL,
    trigger_type            TEXT NOT NULL,
    trigger_reason          TEXT NOT NULL,
    pre_rebalancing_state   JSONB NOT NULL DEFAULT '{}',
    executed_operations     JSONB NOT NULL DEFAULT '[]',
    post_rebalancing_state  JSONB NOT NULL DEFAULT '{}',
    outcome                 TEXT NOT NULL DEFAULT 'pending',
    error_message           TEXT,
    started_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at            TIMESTAMPTZ
);
CREATE INDEX idx_defi_rebalancing_strategy ON defi_rebalancing_audit(strategy_id, started_at DESC);

-- ── Task 3: Yield Distribution ────────────────────────────────────────────────

CREATE TABLE defi_yield_source_records (
    source_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id                 TEXT NOT NULL,
    source_type                 yield_source_type NOT NULL,
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    gross_yield                 NUMERIC(30,8) NOT NULL DEFAULT 0,
    gas_fees_deducted           NUMERIC(30,8) NOT NULL DEFAULT 0,
    platform_management_fee     NUMERIC(30,8) NOT NULL DEFAULT 0,
    protocol_fees_deducted      NUMERIC(30,8) NOT NULL DEFAULT 0,
    net_distributable_yield     NUMERIC(30,8) NOT NULL DEFAULT 0,
    is_realized                 BOOLEAN NOT NULL DEFAULT FALSE,
    recorded_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE defi_yield_tier_configs (
    tier_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id      UUID NOT NULL,
    tier_name       TEXT NOT NULL,
    min_balance     NUMERIC(30,8) NOT NULL DEFAULT 0,
    max_balance     NUMERIC(30,8),
    annual_rate     DOUBLE PRECISION NOT NULL,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_tier_configs_product ON defi_yield_tier_configs(product_id);

CREATE TABLE defi_yield_accruals (
    accrual_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id              UUID NOT NULL,
    product_id              UUID NOT NULL,
    period_start            TIMESTAMPTZ NOT NULL,
    period_end              TIMESTAMPTZ NOT NULL,
    opening_balance         NUMERIC(30,8) NOT NULL DEFAULT 0,
    pro_rata_share          DOUBLE PRECISION NOT NULL DEFAULT 0,
    yield_source_type       yield_source_type NOT NULL,
    rate_applied            DOUBLE PRECISION NOT NULL DEFAULT 0,
    yield_amount            NUMERIC(30,8) NOT NULL DEFAULT 0,
    fiat_equivalent         NUMERIC(30,8) NOT NULL DEFAULT 0,
    is_compound_reinvested  BOOLEAN NOT NULL DEFAULT FALSE,
    credited_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_yield_accruals_account ON defi_yield_accruals(account_id, credited_at DESC);

CREATE TABLE defi_treasury_yield_records (
    record_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    gross_yield                 NUMERIC(30,8) NOT NULL DEFAULT 0,
    management_fee_obligation   NUMERIC(30,8) NOT NULL DEFAULT 0,
    net_treasury_yield          NUMERIC(30,8) NOT NULL DEFAULT 0,
    source_breakdown            JSONB NOT NULL DEFAULT '{}',
    credited_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE defi_yield_reconciliation (
    reconciliation_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cycle_id                        UUID NOT NULL,
    period_start                    TIMESTAMPTZ NOT NULL,
    period_end                      TIMESTAMPTZ NOT NULL,
    total_net_distributable         NUMERIC(30,8) NOT NULL DEFAULT 0,
    total_distributed_to_accounts   NUMERIC(30,8) NOT NULL DEFAULT 0,
    total_distributed_to_treasury   NUMERIC(30,8) NOT NULL DEFAULT 0,
    rounding_discrepancy            NUMERIC(30,8) NOT NULL DEFAULT 0,
    is_balanced                     BOOLEAN NOT NULL DEFAULT TRUE,
    discrepancy_exceeds_tolerance   BOOLEAN NOT NULL DEFAULT FALSE,
    reconciled_at                   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE defi_effective_yield_rates (
    rate_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id      UUID NOT NULL,
    raw_rate        DOUBLE PRECISION NOT NULL DEFAULT 0,
    smoothed_rate   DOUBLE PRECISION NOT NULL DEFAULT 0,
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_yield_rates_product ON defi_effective_yield_rates(product_id, computed_at DESC);

-- ── Task 4: Regulatory Compliance ────────────────────────────────────────────

CREATE TABLE defi_regulatory_activity_log (
    entry_id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 TEXT NOT NULL,
    operation_type          defi_operation_type NOT NULL,
    regulatory_category     defi_regulatory_category NOT NULL,
    protocol_id             TEXT NOT NULL,
    amount                  NUMERIC(30,8) NOT NULL DEFAULT 0,
    asset_code              TEXT NOT NULL,
    jurisdiction            TEXT NOT NULL,
    reporting_obligations   TEXT[] NOT NULL DEFAULT '{}',
    transaction_ref         TEXT NOT NULL,
    executed_at             TIMESTAMPTZ NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_reg_activity_user ON defi_regulatory_activity_log(user_id, executed_at DESC);
CREATE INDEX idx_defi_reg_activity_jurisdiction ON defi_regulatory_activity_log(jurisdiction, executed_at DESC);

CREATE TABLE defi_compliance_thresholds (
    threshold_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    activity_type           TEXT NOT NULL,
    jurisdiction            TEXT NOT NULL,
    threshold_amount        NUMERIC(30,8) NOT NULL,
    threshold_period_days   INT NOT NULL DEFAULT 30,
    reporting_obligation    TEXT NOT NULL,
    is_active               BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE defi_regulatory_reports (
    report_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    report_type         defi_report_type NOT NULL,
    jurisdiction        TEXT NOT NULL,
    period_start        TIMESTAMPTZ NOT NULL,
    period_end          TIMESTAMPTZ NOT NULL,
    filing_status       report_filing_status NOT NULL DEFAULT 'draft',
    filing_deadline     TIMESTAMPTZ NOT NULL,
    report_data         JSONB NOT NULL DEFAULT '{}',
    generated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    generated_by        TEXT NOT NULL,
    reviewed_by         TEXT,
    approved_by         TEXT,
    filed_at            TIMESTAMPTZ,
    filing_channel      TEXT,
    acknowledgement_ref TEXT,
    download_url        TEXT
);
CREATE INDEX idx_defi_reg_reports_deadline ON defi_regulatory_reports(filing_deadline ASC);

CREATE TABLE defi_compliance_audit_trail (
    entry_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type      TEXT NOT NULL,
    description     TEXT NOT NULL,
    actor           TEXT NOT NULL,
    metadata        JSONB NOT NULL DEFAULT '{}',
    entry_hash      TEXT NOT NULL,
    previous_hash   TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_compliance_audit_created ON defi_compliance_audit_trail(created_at ASC);

CREATE TABLE defi_regulatory_changes (
    change_id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    jurisdiction                    TEXT NOT NULL,
    title                           TEXT NOT NULL,
    description                     TEXT NOT NULL,
    effective_date                  TIMESTAMPTZ NOT NULL,
    required_platform_adaptations   TEXT NOT NULL,
    implementation_status           regulatory_change_status NOT NULL DEFAULT 'identified',
    implementation_notes            TEXT,
    recorded_by                     TEXT NOT NULL,
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_defi_reg_changes_effective ON defi_regulatory_changes(effective_date ASC);
