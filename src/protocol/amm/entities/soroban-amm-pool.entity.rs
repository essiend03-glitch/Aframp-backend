CREATE TABLE soroban_amm_pools (
    id UUID PRIMARY KEY,
    contract_hash VARCHAR(255) NOT NULL,
    asset_a VARCHAR(64) NOT NULL,
    asset_b VARCHAR(64) NOT NULL,

    total_value_locked NUMERIC(30,7) NOT NULL,

    pool_share_balance NUMERIC(30,7) NOT NULL,

    liquidity_depth NUMERIC(30,7) NOT NULL,

    slip_margin_bps INTEGER NOT NULL,

    active BOOLEAN DEFAULT TRUE,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);