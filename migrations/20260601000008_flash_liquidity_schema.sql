-- #488 Flash Liquidity Provisioning & On-Chain Credit Facility Schemas
-- Tables: credit_facilities, flash_liquidity_draws, collateral_health_logs

-- ── Credit facilities ─────────────────────────────────────────────────────────

CREATE TYPE facility_status AS ENUM ('active', 'suspended', 'exhausted', 'closed');

CREATE TABLE credit_facilities (
    facility_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lender_name             VARCHAR(120)   NOT NULL,
    lender_api_endpoint     TEXT           NOT NULL,
    max_drawdown_amount     NUMERIC(28,7)  NOT NULL,
    current_utilization     NUMERIC(28,7)  NOT NULL DEFAULT 0,
    -- Variable interest rate in basis points per day
    interest_rate_bps_daily NUMERIC(10,4)  NOT NULL,
    -- Required debt-to-collateral ratio (e.g. 1.50 = 150 %)
    required_dcr            NUMERIC(8,4)   NOT NULL DEFAULT 1.50,
    collateral_asset        VARCHAR(20)    NOT NULL DEFAULT 'USDC',
    status                  facility_status NOT NULL DEFAULT 'active',
    created_at              TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_facility_utilization
        CHECK (current_utilization <= max_drawdown_amount)
);

CREATE INDEX idx_credit_facilities_status ON credit_facilities(status);

-- ── Flash liquidity draws ─────────────────────────────────────────────────────

CREATE TYPE draw_status AS ENUM (
    'pending', 'collateral_locked', 'disbursed', 'repaid', 'defaulted', 'rolled_back'
);

CREATE TABLE flash_liquidity_draws (
    draw_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    facility_id             UUID           NOT NULL REFERENCES credit_facilities(facility_id),
    -- Parent settlement transaction that triggered the draw
    parent_settlement_id    UUID           NOT NULL,
    corridor                VARCHAR(20)    NOT NULL,   -- e.g. 'NGN/USD'
    draw_amount             NUMERIC(28,7)  NOT NULL,
    collateral_amount       NUMERIC(28,7)  NOT NULL,
    collateral_asset        VARCHAR(20)    NOT NULL,
    -- On-chain escrow account hash (Stellar)
    escrow_account_hash     VARCHAR(128),
    -- XDR signature of the collateral lock transaction
    lock_xdr_signature      TEXT,
    status                  draw_status    NOT NULL DEFAULT 'pending',
    -- Maturity timestamp for automated repayment
    repayment_due_at        TIMESTAMPTZ    NOT NULL,
    repaid_at               TIMESTAMPTZ,
    interest_accrued        NUMERIC(28,7)  NOT NULL DEFAULT 0,
    error_message           TEXT,
    created_at              TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_draws_facility   ON flash_liquidity_draws(facility_id);
CREATE INDEX idx_draws_settlement ON flash_liquidity_draws(parent_settlement_id);
CREATE INDEX idx_draws_status     ON flash_liquidity_draws(status);
CREATE INDEX idx_draws_repayment  ON flash_liquidity_draws(repayment_due_at)
    WHERE status NOT IN ('repaid', 'defaulted', 'rolled_back');

-- ── Collateral health logs ────────────────────────────────────────────────────

CREATE TABLE collateral_health_logs (
    log_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    draw_id             UUID           NOT NULL REFERENCES flash_liquidity_draws(draw_id),
    -- Current collateral value in USD equivalent
    collateral_value_usd NUMERIC(28,7) NOT NULL,
    debt_amount_usd     NUMERIC(28,7)  NOT NULL,
    -- Health factor = collateral_value / (debt * required_dcr)
    health_factor       NUMERIC(10,6)  NOT NULL,
    -- Liquidation boundary (health_factor < 1.0 triggers liquidation)
    near_liquidation    BOOLEAN        NOT NULL DEFAULT FALSE,
    -- Action taken by circuit breaker (NULL = no action)
    circuit_breaker_action VARCHAR(64),
    evaluated_at        TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_collateral_draw     ON collateral_health_logs(draw_id);
CREATE INDEX idx_collateral_health   ON collateral_health_logs(health_factor);
CREATE INDEX idx_collateral_near_liq ON collateral_health_logs(near_liquidation)
    WHERE near_liquidation = TRUE;
