CREATE TABLE arbitrage_yield_ledger (
    id UUID PRIMARY KEY,

    route VARCHAR(255),

    tx_hash VARCHAR(255),

    captured_yield NUMERIC(30,7),

    slippage_saved NUMERIC(30,7),

    created_at TIMESTAMP NOT NULL
);