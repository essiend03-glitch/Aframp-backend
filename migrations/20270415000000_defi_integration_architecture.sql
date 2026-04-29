-- DeFi Integration Architecture & Protocol Selection (Issue #370)
-- Comprehensive database schema for DeFi protocol integration, yield strategies,
-- savings products, and Stellar AMM integration

-- ── DeFi Protocol Types ───────────────────────────────────────────────────────

CREATE TYPE risk_tier AS ENUM ('tier1', 'tier2', 'tier3');
CREATE TYPE protocol_type AS ENUM ('dex', 'amm', 'lending', 'liquidity_mining', 'yield_farming');
CREATE TYPE governance_status AS ENUM ('pending', 'approved', 'rejected', 'suspended');
CREATE TYPE position_status AS ENUM ('active', 'withdrawing', 'closed', 'emergency_withdrawal');
CREATE TYPE strategy_type AS ENUM ('single_protocol', 'multi_protocol', 'dynamic_allocation');
CREATE TYPE strategy_status AS ENUM ('draft', 'pending_approval', 'active', 'paused', 'deprecated');
CREATE TYPE allocation_status AS ENUM ('active', 'rebalancing', 'paused', 'closed');
CREATE TYPE approval_type AS ENUM ('approve', 'reject');
CREATE TYPE committee_role AS ENUM ('chair', 'member', 'observer');
CREATE TYPE governance_entity_type AS ENUM ('protocol', 'strategy', 'committee_member', 'approval_record');
CREATE TYPE governance_action AS ENUM ('submit', 'approve', 'reject', 'activate', 'suspend', 'modify', 'delete');
CREATE TYPE strategy_change_type AS ENUM ('allocation_change', 'risk_parameter_change', 'rebalancing_frequency_change', 'protocol_addition', 'protocol_removal', 'yield_rate_target_change', 'emergency_suspension');
CREATE TYPE circuit_breaker_trigger AS ENUM ('tvl_drop', 'health_score_drop', 'smart_contract_pause', 'abnormal_volume_spike', 'yield_rate_collapse');
CREATE TYPE savings_product_type AS ENUM ('flexible', 'fixed_term');
CREATE TYPE savings_account_status AS ENUM ('active', 'withdrawing', 'closed');
CREATE TYPE withdrawal_type AS ENUM ('full', 'partial');
CREATE TYPE amm_pool_status AS ENUM ('active', 'inactive', 'maintenance');

-- ── DeFi Protocol Configuration ───────────────────────────────────────────────

CREATE TABLE defi_protocols (
    protocol_id              VARCHAR(50) PRIMARY KEY,
    protocol_name            VARCHAR(100) NOT NULL,
    protocol_type            protocol_type NOT NULL,
    risk_tier                risk_tier NOT NULL,
    is_active                BOOLEAN NOT NULL DEFAULT true,
    max_exposure_percentage  NUMERIC(5,2) NOT NULL,
    max_single_transaction_amount NUMERIC(28,8) NOT NULL,
    min_deposit_amount       NUMERIC(28,8) NOT NULL,
    max_deposit_amount       NUMERIC(28,8) NOT NULL,
    default_slippage_tolerance NUMERIC(5,4) NOT NULL,
    health_check_interval_secs BIGINT NOT NULL,
    tvl_score                NUMERIC(3,2) NOT NULL,
    age_score                NUMERIC(3,2) NOT NULL,
    audit_score              NUMERIC(3,2) NOT NULL,
    team_score               NUMERIC(3,2) NOT NULL,
    codebase_score           NUMERIC(3,2) NOT NULL,
    governance_score         NUMERIC(3,2) NOT NULL,
    compliance_score         NUMERIC(3,2) NOT NULL,
    ecosystem_score          NUMERIC(3,2) NOT NULL,
    total_score              NUMERIC(3,2) NOT NULL,
    governance_status        governance_status NOT NULL DEFAULT 'pending',
    evaluation_summary       TEXT,
    protocol_metadata        JSONB,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_defi_protocols_active ON defi_protocols(is_active) WHERE is_active = true;
CREATE INDEX idx_defi_protocols_risk_tier ON defi_protocols(risk_tier);
CREATE INDEX idx_defi_protocols_type ON defi_protocols(protocol_type);

-- ── DeFi Protocol Positions ───────────────────────────────────────────────────

CREATE TABLE defi_positions (
    position_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id              VARCHAR(50) NOT NULL REFERENCES defi_protocols(protocol_id),
    asset_code               VARCHAR(20) NOT NULL,
    deposited_amount         NUMERIC(28,8) NOT NULL,
    current_value            NUMERIC(28,8) NOT NULL,
    yield_earned             NUMERIC(28,8) NOT NULL DEFAULT 0,
    effective_yield_rate    NUMERIC(8,6) NOT NULL,
    position_opened_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    position_status          position_status NOT NULL DEFAULT 'active',
    protocol_position_id     VARCHAR(100), -- ID in the external protocol
    metadata                 JSONB
);

CREATE INDEX idx_defi_positions_protocol ON defi_positions(protocol_id);
CREATE INDEX idx_defi_positions_asset ON defi_positions(asset_code);
CREATE INDEX idx_defi_positions_status ON defi_positions(position_status);
CREATE INDEX idx_defi_positions_updated ON defi_positions(last_updated_at);

-- ── Yield Strategies ───────────────────────────────────────────────────────────

CREATE TABLE yield_strategies (
    strategy_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_name            VARCHAR(200) NOT NULL,
    description              TEXT,
    strategy_type            strategy_type NOT NULL,
    target_yield_rate        NUMERIC(8,6) NOT NULL,
    min_acceptable_yield_rate NUMERIC(8,6) NOT NULL,
    max_acceptable_risk_score NUMERIC(3,2) NOT NULL,
    total_allocated_amount   NUMERIC(28,8) NOT NULL DEFAULT 0,
    max_allocation_limit     NUMERIC(28,8) NOT NULL,
    rebalancing_frequency_secs BIGINT NOT NULL,
    rebalancing_triggers    JSONB NOT NULL, -- RebalancingTriggers
    strategy_status          strategy_status NOT NULL DEFAULT 'draft',
    governance_approval_id  UUID,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT chk_yield_rates CHECK (target_yield_rate > min_acceptable_yield_rate),
    CONSTRAINT chk_allocation_limit CHECK (total_allocated_amount <= max_allocation_limit)
);

CREATE INDEX idx_yield_strategies_status ON yield_strategies(strategy_status);
CREATE INDEX idx_yield_strategies_type ON yield_strategies(strategy_type);
CREATE INDEX idx_yield_strategies_updated ON yield_strategies(updated_at);

-- ── Strategy Allocations ─────────────────────────────────────────────────────

CREATE TABLE strategy_allocations (
    allocation_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id              UUID NOT NULL REFERENCES yield_strategies(strategy_id),
    protocol_id              VARCHAR(50) NOT NULL REFERENCES defi_protocols(protocol_id),
    target_allocation_percentage NUMERIC(5,2) NOT NULL,
    current_allocation_amount NUMERIC(28,8) NOT NULL DEFAULT 0,
    min_allocation_percentage NUMERIC(5,2) NOT NULL,
    max_allocation_percentage NUMERIC(5,2) NOT NULL,
    last_rebalanced_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    allocation_status       allocation_status NOT NULL DEFAULT 'active',
    
    CONSTRAINT chk_allocation_percentages CHECK (
        target_allocation_percentage >= min_allocation_percentage AND
        target_allocation_percentage <= max_allocation_percentage AND
        target_allocation_percentage >= 0 AND target_allocation_percentage <= 100
    ),
    
    CONSTRAINT unique_strategy_protocol UNIQUE (strategy_id, protocol_id)
);

CREATE INDEX idx_strategy_allocations_strategy ON strategy_allocations(strategy_id);
CREATE INDEX idx_strategy_allocations_protocol ON strategy_allocations(protocol_id);
CREATE INDEX idx_strategy_allocations_status ON strategy_allocations(allocation_status);

-- ── Strategy Performance Records ─────────────────────────────────────────────────

CREATE TABLE strategy_performance (
    performance_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id              UUID NOT NULL REFERENCES yield_strategies(strategy_id),
    period_start             TIMESTAMPTZ NOT NULL,
    period_end               TIMESTAMPTZ NOT NULL,
    opening_allocation       NUMERIC(28,8) NOT NULL,
    closing_allocation       NUMERIC(28,8) NOT NULL,
    yield_earned             NUMERIC(28,8) NOT NULL,
    effective_yield_rate     NUMERIC(8,6) NOT NULL,
    max_drawdown             NUMERIC(5,2) NOT NULL,
    risk_score_at_end        NUMERIC(3,2) NOT NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT chk_period_order CHECK (period_end > period_start),
    CONSTRAINT chk_performance_positive CHECK (closing_allocation >= opening_allocation)
);

CREATE INDEX idx_strategy_performance_strategy_period ON strategy_performance(strategy_id, period_start DESC);

-- ── Strategy Risk Parameters ───────────────────────────────────────────────────

CREATE TABLE strategy_risk_parameters (
    parameter_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id              UUID NOT NULL REFERENCES yield_strategies(strategy_id),
    max_single_protocol_exposure_pct NUMERIC(5,2) NOT NULL,
    max_correlation_between_protocols NUMERIC(5,2) NOT NULL,
    max_acceptable_impermanent_loss_pct NUMERIC(5,2) NOT NULL,
    circuit_breaker_tvl_drop_threshold NUMERIC(5,2) NOT NULL,
    emergency_withdrawal_trigger_conditions JSONB NOT NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_strategy_risk UNIQUE (strategy_id)
);

-- ── Governance Committee Members ───────────────────────────────────────────────

CREATE TABLE governance_committee_members (
    user_id                  VARCHAR(100) PRIMARY KEY,
    name                     VARCHAR(200) NOT NULL,
    email                    VARCHAR(255) NOT NULL,
    role                     committee_role NOT NULL DEFAULT 'member',
    is_active                BOOLEAN NOT NULL DEFAULT true,
    joined_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expertise_areas          TEXT[],
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_governance_members_active ON governance_committee_members(is_active) WHERE is_active = true;
CREATE INDEX idx_governance_members_role ON governance_committee_members(role);

-- ── Strategy Governance Approvals ─────────────────────────────────────────────

CREATE TABLE strategy_governance_approvals (
    record_id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id              UUID NOT NULL REFERENCES yield_strategies(strategy_id),
    submitted_by             VARCHAR(100) NOT NULL REFERENCES governance_committee_members(user_id),
    submitted_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    required_approvals       INTEGER NOT NULL,
    received_approvals       INTEGER NOT NULL DEFAULT 0,
    approval_status          governance_status NOT NULL DEFAULT 'pending',
    rejection_reason         TEXT,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_strategy_approval UNIQUE (strategy_id)
);

CREATE INDEX idx_strategy_governance_status ON strategy_governance_approvals(approval_status);
CREATE INDEX idx_strategy_governance_submitted ON strategy_governance_approvals(submitted_at DESC);

-- ── Individual Governance Approvals ─────────────────────────────────────────────

CREATE TABLE governance_approvals (
    approval_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    record_id                UUID NOT NULL REFERENCES strategy_governance_approvals(record_id),
    committee_member         VARCHAR(100) NOT NULL REFERENCES governance_committee_members(user_id),
    approved_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    justification            TEXT NOT NULL,
    approval_type            approval_type NOT NULL,
    
    CONSTRAINT unique_member_approval UNIQUE (record_id, committee_member)
);

CREATE INDEX idx_governance_approvals_record ON governance_approvals(record_id);
CREATE INDEX idx_governance_approvals_member ON governance_approvals(committee_member);

-- ── Circuit Breaker Trips ─────────────────────────────────────────────────────

CREATE TABLE circuit_breaker_trips (
    trip_id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id              VARCHAR(50) NOT NULL REFERENCES defi_protocols(protocol_id),
    trigger                  circuit_breaker_trigger NOT NULL,
    reason                   TEXT NOT NULL,
    tripped_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at              TIMESTAMPTZ,
    resolved_by              VARCHAR(100) REFERENCES governance_committee_members(user_id),
    resolution_reason        TEXT
);

CREATE INDEX idx_circuit_breaker_protocol ON circuit_breaker_trips(protocol_id);
CREATE INDEX idx_circuit_breaker_tripped ON circuit_breaker_trips(tripped_at DESC);
CREATE INDEX idx_circuit_breaker_resolved ON circuit_breaker_trips(resolved_at) WHERE resolved_at IS NOT NULL;

-- ── Rebalancing Events ───────────────────────────────────────────────────────

CREATE TABLE rebalancing_events (
    event_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id              UUID NOT NULL REFERENCES yield_strategies(strategy_id),
    trigger_reason           VARCHAR(100) NOT NULL,
    pre_rebalancing_allocations JSONB NOT NULL,
    post_rebalancing_allocations JSONB NOT NULL,
    transaction_details      JSONB NOT NULL,
    started_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at             TIMESTAMPTZ,
    status                   VARCHAR(20) NOT NULL DEFAULT 'in_progress',
    error_message            TEXT
);

CREATE INDEX idx_rebalancing_strategy ON rebalancing_events(strategy_id);
CREATE INDEX idx_rebalancing_started ON rebalancing_events(started_at DESC);
CREATE INDEX idx_rebalancing_status ON rebalancing_events(status);

-- ── cNGN Savings Products ─────────────────────────────────────────────────────

CREATE TABLE cngn_savings_products (
    product_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_name             VARCHAR(200) NOT NULL,
    description              TEXT,
    product_type             savings_product_type NOT NULL,
    minimum_deposit_amount   NUMERIC(28,8) NOT NULL,
    maximum_deposit_amount   NUMERIC(28,8) NOT NULL,
    lock_up_period_hours     BIGINT NOT NULL DEFAULT 0,
    early_withdrawal_penalty_pct NUMERIC(5,2) NOT NULL DEFAULT 0,
    target_yield_rate        NUMERIC(8,6) NOT NULL,
    yield_rate_source        VARCHAR(20) NOT NULL DEFAULT 'variable', -- 'fixed' or 'variable'
    underlying_strategy_id   UUID REFERENCES yield_strategies(strategy_id),
    yield_rate_floor         NUMERIC(8,6),
    yield_rate_ceil          NUMERIC(8,6),
    product_status           VARCHAR(20) NOT NULL DEFAULT 'active',
    risk_disclosure_version  VARCHAR(20) NOT NULL DEFAULT '1.0',
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT chk_deposit_limits CHECK (minimum_deposit_amount <= maximum_deposit_amount),
    CONSTRAINT chk_penalty_range CHECK (early_withdrawal_penalty_pct >= 0 AND early_withdrawal_penalty_pct <= 100)
);

CREATE INDEX idx_cngn_savings_products_status ON cngn_savings_products(product_status);
CREATE INDEX idx_cngn_savings_products_type ON cngn_savings_products(product_type);

-- ── cNGN Savings Accounts ─────────────────────────────────────────────────────

CREATE TABLE cngn_savings_accounts (
    account_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id                UUID NOT NULL,
    product_id               UUID NOT NULL REFERENCES cngn_savings_products(product_id),
    deposited_amount         NUMERIC(28,8) NOT NULL DEFAULT 0,
    current_balance          NUMERIC(28,8) NOT NULL DEFAULT 0,
    accrued_yield_to_date    NUMERIC(28,8) NOT NULL DEFAULT 0,
    current_yield_rate       NUMERIC(8,6) NOT NULL,
    deposit_timestamp        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_yield_accrual_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    withdrawal_eligibility_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    account_status           savings_account_status NOT NULL DEFAULT 'active',
    risk_disclosure_accepted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    risk_disclosure_ip_address INET,
    
    CONSTRAINT unique_wallet_product UNIQUE (wallet_id, product_id)
);

CREATE INDEX idx_cngn_savings_accounts_wallet ON cngn_savings_accounts(wallet_id);
CREATE INDEX idx_cngn_savings_accounts_product ON cngn_savings_accounts(product_id);
CREATE INDEX idx_cngn_savings_accounts_status ON cngn_savings_accounts(account_status);
CREATE INDEX idx_cngn_savings_accounts_eligibility ON cngn_savings_accounts(withdrawal_eligibility_timestamp);

-- ── Yield Accrual Records ─────────────────────────────────────────────────────

CREATE TABLE yield_accrual_records (
    accrual_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id               UUID NOT NULL REFERENCES cngn_savings_accounts(account_id),
    accrual_period_start     TIMESTAMPTZ NOT NULL,
    accrual_period_end       TIMESTAMPTZ NOT NULL,
    opening_balance          NUMERIC(28,8) NOT NULL,
    yield_rate_applied       NUMERIC(8,6) NOT NULL,
    yield_amount_earned      NUMERIC(28,8) NOT NULL,
    accrual_timestamp        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_yield_accrual_account ON yield_accrual_records(account_id);
CREATE INDEX idx_yield_accrual_period ON yield_accrual_records(accrual_period_start DESC);

-- ── Withdrawal Requests ───────────────────────────────────────────────────────

CREATE TABLE withdrawal_requests (
    request_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id               UUID NOT NULL REFERENCES cngn_savings_accounts(account_id),
    requested_amount         NUMERIC(28,8) NOT NULL,
    withdrawal_type          withdrawal_type NOT NULL,
    early_withdrawal_flag    BOOLEAN NOT NULL DEFAULT false,
    penalty_amount           NUMERIC(28,8) NOT NULL DEFAULT 0,
    net_withdrawal_amount    NUMERIC(28,8) NOT NULL,
    request_timestamp        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    settlement_timestamp     TIMESTAMPTZ,
    status                   VARCHAR(20) NOT NULL DEFAULT 'pending',
    transaction_hash         VARCHAR(100)
);

CREATE INDEX idx_withdrawal_requests_account ON withdrawal_requests(account_id);
CREATE INDEX idx_withdrawal_requests_status ON withdrawal_requests(status);
CREATE INDEX idx_withdrawal_requests_timestamp ON withdrawal_requests(request_timestamp DESC);

-- ── Stellar AMM Pools ─────────────────────────────────────────────────────────

CREATE TABLE stellar_amm_pools (
    pool_id                  VARCHAR(100) PRIMARY KEY, -- Stellar pool ID
    asset_a_code             VARCHAR(20) NOT NULL,
    asset_a_issuer           VARCHAR(100),
    asset_b_code             VARCHAR(20) NOT NULL,
    asset_b_issuer           VARCHAR(100),
    total_pool_shares        NUMERIC(28,8) NOT NULL,
    asset_a_reserves         NUMERIC(28,8) NOT NULL,
    asset_b_reserves         NUMERIC(28,8) NOT NULL,
    current_price            NUMERIC(20,8) NOT NULL,
    trading_fee_bps          INTEGER NOT NULL,
    pool_status              amm_pool_status NOT NULL DEFAULT 'active',
    tvl_24h_ago              NUMERIC(28,8),
    volume_24h               NUMERIC(28,8) NOT NULL DEFAULT 0,
    fees_24h                 NUMERIC(28,8) NOT NULL DEFAULT 0,
    last_updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    discovered_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_stellar_amm_pools_assets ON stellar_amm_pools(asset_a_code, asset_b_code);
CREATE INDEX idx_stellar_amm_pools_status ON stellar_amm_pools(pool_status);
CREATE INDEX idx_stellar_amm_pools_updated ON stellar_amm_pools(last_updated_at);

-- ── AMM Liquidity Positions ─────────────────────────────────────────────────

CREATE TABLE amm_liquidity_positions (
    position_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id                  VARCHAR(100) NOT NULL REFERENCES stellar_amm_pools(pool_id),
    strategy_id              UUID REFERENCES yield_strategies(strategy_id),
    shares_owned             NUMERIC(28,8) NOT NULL,
    asset_a_deposited        NUMERIC(28,8) NOT NULL,
    asset_b_deposited        NUMERIC(28,8) NOT NULL,
    initial_share_price      NUMERIC(20,8) NOT NULL,
    current_share_price      NUMERIC(20,8) NOT NULL,
    unrealized_yield         NUMERIC(28,8) NOT NULL DEFAULT 0,
    impermanent_loss         NUMERIC(28,8) NOT NULL DEFAULT 0,
    fee_income_earned        NUMERIC(28,8) NOT NULL DEFAULT 0,
    position_opened_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_valuation_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    position_status          VARCHAR(20) NOT NULL DEFAULT 'active'
);

CREATE INDEX idx_amm_positions_pool ON amm_liquidity_positions(pool_id);
CREATE INDEX idx_amm_positions_strategy ON amm_liquidity_positions(strategy_id);
CREATE INDEX idx_amm_positions_status ON amm_liquidity_positions(position_status);

-- ── AMM Position Value Snapshots ───────────────────────────────────────────────

CREATE TABLE amm_position_snapshots (
    snapshot_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    position_id              UUID NOT NULL REFERENCES amm_liquidity_positions(position_id),
    pool_share_price         NUMERIC(20,8) NOT NULL,
    asset_a_value            NUMERIC(28,8) NOT NULL,
    asset_b_value            NUMERIC(28,8) NOT NULL,
    total_position_value     NUMERIC(28,8) NOT NULL,
    unrealized_yield         NUMERIC(28,8) NOT NULL,
    impermanent_loss         NUMERIC(28,8) NOT NULL,
    fee_income_earned        NUMERIC(28,8) NOT NULL,
    snapshotted_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_amm_snapshots_position ON amm_position_snapshots(position_id);
CREATE INDEX idx_amm_snapshots_timestamp ON amm_position_snapshots(snapshotted_at DESC);

-- ── Yield Rate History ───────────────────────────────────────────────────────

CREATE TABLE yield_rate_history (
    history_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id               UUID NOT NULL REFERENCES cngn_savings_products(product_id),
    yield_rate               NUMERIC(8,6) NOT NULL,
    rate_source              VARCHAR(50) NOT NULL,
    recorded_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_yield_rate_history_product ON yield_rate_history(product_id);
CREATE INDEX idx_yield_rate_history_timestamp ON yield_rate_history(recorded_at DESC);

-- ── Governance Audit Log ─────────────────────────────────────────────────────

CREATE TABLE governance_audit_log (
    log_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type              governance_entity_type NOT NULL,
    entity_id                VARCHAR(100) NOT NULL,
    action                   governance_action NOT NULL,
    performed_by             VARCHAR(100) NOT NULL,
    performed_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    details                  JSONB NOT NULL,
    ip_address               INET,
    user_agent               TEXT
);

CREATE INDEX idx_governance_audit_entity ON governance_audit_log(entity_type, entity_id);
CREATE INDEX idx_governance_audit_action ON governance_audit_log(action);
CREATE INDEX idx_governance_audit_performed ON governance_audit_log(performed_at DESC);

-- ── Seed Data ───────────────────────────────────────────────────────────────

-- Insert initial governance committee members
INSERT INTO governance_committee_members (user_id, name, email, role, expertise_areas)
VALUES 
    ('governance-chair-001', 'Dr. Sarah Chen', 'sarah.chen@aframp.io', 'chair', ARRAY['defi', 'risk_management', 'compliance']),
    ('governance-member-001', 'Michael Okonkwo', 'michael.okonkwo@aframp.io', 'member', ARRAY['stellar', 'amm', 'liquidity']),
    ('governance-member-002', 'Amara Diallo', 'amara.diallo@aframp.io', 'member', ARRAY['treasury', 'yield_strategies', 'risk_controls']),
    ('governance-member-003', 'James Mwangi', 'james.mwangi@aframp.io', 'member', ARRAY['regulatory', 'compliance', 'audit']),
    ('governance-member-004', 'Fatima Al-Rashid', 'fatima.al-rashid@aframp.io', 'member', ARRAY['smart_contracts', 'security', 'protocol_integration']);

-- Insert initial DeFi protocols (Stellar DEX and AMM as Tier 1)
INSERT INTO defi_protocols (
    protocol_id, protocol_name, protocol_type, risk_tier, max_exposure_percentage,
    max_single_transaction_amount, min_deposit_amount, max_deposit_amount,
    default_slippage_tolerance, health_check_interval_secs,
    tvl_score, age_score, audit_score, team_score, codebase_score,
    governance_score, compliance_score, ecosystem_score, total_score,
    governance_status, evaluation_summary
) VALUES 
(
    'stellar_dex', 'Stellar Decentralized Exchange', 'dex', 'tier1', 25.0,
    1000000.0, 100.0, 50000000.0,
    0.0100, 300,
    0.95, 0.90, 0.95, 0.90, 0.95,
    0.85, 0.90, 0.95, 0.91,
    'approved', 'Native Stellar DEX with excellent security and liquidity'
),
(
    'stellar_amm', 'Stellar Automated Market Maker', 'amm', 'tier1', 20.0,
    1000000.0, 100.0, 50000000.0,
    0.0100, 300,
    0.90, 0.85, 0.90, 0.85, 0.90,
    0.80, 0.85, 0.90, 0.87,
    'approved', 'Native Stellar AMM with constant product formula'
);

-- Insert initial cNGN savings products
INSERT INTO cngn_savings_products (
    product_name, description, product_type, minimum_deposit_amount,
    maximum_deposit_amount, lock_up_period_hours, early_withdrawal_penalty_pct,
    target_yield_rate, yield_rate_source, yield_rate_floor, yield_rate_ceil
) VALUES 
(
    'cNGN Flexible Savings', 'Flexible cNGN savings with competitive yield and no lock-up period',
    'flexible', 1000.0, 10000000.0, 0, 0.0,
    0.085, 'variable', 0.050, 0.150
),
(
    'cNGN Fixed-Term Savings', 'Fixed-term cNGN savings with higher yield rates',
    'fixed_term', 1000.0, 10000000.0, 2160, 5.0, -- 90 days lock-up, 5% penalty
    0.120, 'variable', 0.080, 0.180
);

-- ── Constraints and Triggers ─────────────────────────────────────────────────

-- Ensure strategy allocation percentages sum to 100%
CREATE OR REPLACE FUNCTION validate_strategy_allocation_sum()
RETURNS TRIGGER AS $$
BEGIN
    DECLARE
        allocation_sum NUMERIC;
    BEGIN
        SELECT COALESCE(SUM(target_allocation_percentage), 0) INTO allocation_sum
        FROM strategy_allocations
        WHERE strategy_id = NEW.strategy_id;
        
        IF allocation_sum != 100 THEN
            RAISE EXCEPTION 'Strategy allocation percentages must sum to exactly 100%%, got %', allocation_sum;
        END IF;
        
        RETURN NEW;
    END;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_validate_strategy_allocation_sum
    AFTER INSERT OR UPDATE ON strategy_allocations
    FOR EACH ROW EXECUTE FUNCTION validate_strategy_allocation_sum();

-- Update updated_at timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply updated_at trigger to relevant tables
CREATE TRIGGER trigger_defi_protocols_updated_at
    BEFORE UPDATE ON defi_protocols
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_yield_strategies_updated_at
    BEFORE UPDATE ON yield_strategies
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_strategy_risk_parameters_updated_at
    BEFORE UPDATE ON strategy_risk_parameters
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_strategy_governance_approvals_updated_at
    BEFORE UPDATE ON strategy_governance_approvals
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_governance_committee_members_updated_at
    BEFORE UPDATE ON governance_committee_members
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trigger_cngn_savings_products_updated_at
    BEFORE UPDATE ON cngn_savings_products
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ── Views ─────────────────────────────────────────────────────────────────────

-- Strategy overview view
CREATE VIEW strategy_overview AS
SELECT 
    s.strategy_id,
    s.strategy_name,
    s.strategy_type,
    s.strategy_status,
    s.target_yield_rate,
    s.total_allocated_amount,
    s.max_allocation_limit,
    COUNT(sa.allocation_id) as allocation_count,
    COALESCE(SUM(sa.current_allocation_amount), 0) as current_total_allocation,
    s.created_at,
    s.updated_at
FROM yield_strategies s
LEFT JOIN strategy_allocations sa ON s.strategy_id = sa.strategy_id
GROUP BY s.strategy_id, s.strategy_name, s.strategy_type, s.strategy_status,
         s.target_yield_rate, s.total_allocated_amount, s.max_allocation_limit,
         s.created_at, s.updated_at;

-- Protocol exposure view
CREATE VIEW protocol_exposure_overview AS
SELECT 
    p.protocol_id,
    p.protocol_name,
    p.protocol_type,
    p.risk_tier,
    p.max_exposure_percentage,
    COALESCE(SUM(dp.current_value), 0) as current_exposure,
    COUNT(dp.position_id) as active_positions,
    p.is_active
FROM defi_protocols p
LEFT JOIN defi_positions dp ON p.protocol_id = dp.protocol_id AND dp.position_status = 'active'
GROUP BY p.protocol_id, p.protocol_name, p.protocol_type, p.risk_tier,
         p.max_exposure_percentage, p.is_active;

-- Savings product performance view
CREATE VIEW savings_product_performance AS
SELECT 
    sp.product_id,
    sp.product_name,
    sp.product_type,
    sp.target_yield_rate,
    COUNT(sa.account_id) as active_accounts,
    COALESCE(SUM(sa.current_balance), 0) as total_deposits,
    COALESCE(SUM(sa.accrued_yield_to_date), 0) as total_yield_accrued,
    sp.product_status
FROM cngn_savings_products sp
LEFT JOIN cngn_savings_accounts sa ON sp.product_id = sa.product_id AND sa.account_status = 'active'
GROUP BY sp.product_id, sp.product_name, sp.product_type, sp.target_yield_rate, sp.product_status;
