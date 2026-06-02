-- Migration: SOR and Rebalancing tables
CREATE TABLE IF NOT EXISTS liquidity_venues (
    venue_id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    connection_status VARCHAR(50) NOT NULL,
    api_credentials JSONB NOT NULL,
    target_currencies VARCHAR(10)[] NOT NULL,
    daily_volume_limit NUMERIC(20, 7) NOT NULL,
    execution_fee_bps INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS smart_order_executions (
    execution_id UUID PRIMARY KEY,
    primary_transaction_id UUID NOT NULL,
    venue_id UUID NOT NULL REFERENCES liquidity_venues(venue_id),
    child_order_id UUID NOT NULL,
    amount NUMERIC(20, 7) NOT NULL,
    currency VARCHAR(10) NOT NULL,
    slippage_bps INT NOT NULL,
    status VARCHAR(50) NOT NULL,
    execution_time_ms INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS treasury_rebalancing_rules (
    rule_id UUID PRIMARY KEY,
    currency VARCHAR(10) NOT NULL UNIQUE,
    target_percentage NUMERIC(5, 2) NOT NULL,
    min_threshold_percentage NUMERIC(5, 2) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
