CREATE TABLE payout_corridor_iso_map (
    id UUID PRIMARY KEY,

    iso_reference VARCHAR(255) UNIQUE,

    internal_transaction_id UUID,

    ledger_hash VARCHAR(255),

    payout_corridor VARCHAR(128),

    settlement_currency VARCHAR(16),

    created_at TIMESTAMP NOT NULL
);