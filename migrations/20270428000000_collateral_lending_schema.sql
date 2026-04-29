-- Migration: Collateralized Lending Schema (Issue #379)
-- cNGN collateralized lending positions, repayments, collateral adjustments, and liquidation events

CREATE TYPE lending_position_status AS ENUM (
    'active',
    'at_risk',
    'liquidated',
    'repaid',
    'closed'
);

CREATE TYPE collateral_adjustment_type AS ENUM (
    'deposit',
    'withdrawal'
);

-- Core lending position record
CREATE TABLE lending_positions (
    position_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id                   UUID NOT NULL,
    lending_protocol_id         TEXT NOT NULL,
    collateral_asset_code       TEXT NOT NULL,
    collateral_amount           NUMERIC(28, 8) NOT NULL,
    collateral_value_fiat       NUMERIC(28, 8) NOT NULL,
    borrowed_asset_code         TEXT NOT NULL,
    borrowed_amount             NUMERIC(28, 8) NOT NULL,
    borrowed_value_fiat         NUMERIC(28, 8) NOT NULL,
    collateral_ratio            NUMERIC(18, 8) NOT NULL,
    liquidation_threshold_ratio NUMERIC(18, 8) NOT NULL,
    health_factor               NUMERIC(18, 8) NOT NULL,
    interest_rate               NUMERIC(10, 8) NOT NULL,
    interest_accrued            NUMERIC(28, 8) NOT NULL DEFAULT 0,
    status                      lending_position_status NOT NULL DEFAULT 'active',
    opened_at                   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_health_check_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_lending_positions_wallet_id ON lending_positions (wallet_id);
CREATE INDEX idx_lending_positions_status ON lending_positions (status);
CREATE INDEX idx_lending_positions_health_factor ON lending_positions (health_factor) WHERE status = 'active';

-- Loan repayment records
CREATE TABLE loan_repayments (
    repayment_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    position_id             UUID NOT NULL REFERENCES lending_positions (position_id),
    repayment_amount        NUMERIC(28, 8) NOT NULL,
    repayment_asset         TEXT NOT NULL,
    interest_paid           NUMERIC(28, 8) NOT NULL,
    principal_repaid        NUMERIC(28, 8) NOT NULL,
    remaining_balance       NUMERIC(28, 8) NOT NULL,
    transaction_reference   TEXT NOT NULL,
    repaid_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_loan_repayments_position_id ON loan_repayments (position_id);

-- Collateral adjustment records
CREATE TABLE collateral_adjustments (
    adjustment_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    position_id                     UUID NOT NULL REFERENCES lending_positions (position_id),
    adjustment_type                 collateral_adjustment_type NOT NULL,
    adjustment_amount               NUMERIC(28, 8) NOT NULL,
    pre_adjustment_collateral       NUMERIC(28, 8) NOT NULL,
    post_adjustment_collateral      NUMERIC(28, 8) NOT NULL,
    pre_adjustment_health_factor    NUMERIC(18, 8) NOT NULL,
    post_adjustment_health_factor   NUMERIC(18, 8) NOT NULL,
    adjusted_at                     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_collateral_adjustments_position_id ON collateral_adjustments (position_id);

-- Liquidation event records
CREATE TABLE liquidation_events (
    liquidation_id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    position_id                     UUID NOT NULL REFERENCES lending_positions (position_id),
    trigger_health_factor           NUMERIC(18, 8) NOT NULL,
    liquidated_collateral_amount    NUMERIC(28, 8) NOT NULL,
    liquidated_collateral_value     NUMERIC(28, 8) NOT NULL,
    repaid_debt_amount              NUMERIC(28, 8) NOT NULL,
    liquidation_penalty_amount      NUMERIC(28, 8) NOT NULL,
    liquidator_address              TEXT NOT NULL,
    liquidated_at                   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_liquidation_events_position_id ON liquidation_events (position_id);
