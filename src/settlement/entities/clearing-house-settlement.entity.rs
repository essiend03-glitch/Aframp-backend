CREATE TABLE clearing_house_settlements (
    id UUID PRIMARY KEY,

    settlement_batch_id VARCHAR(255),

    corridor VARCHAR(128),

    currency VARCHAR(16),

    settlement_amount NUMERIC(30,7),

    fees NUMERIC(30,7),

    processing_date TIMESTAMP,

    status VARCHAR(32),

    created_at TIMESTAMP NOT NULL
);