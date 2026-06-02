CREATE TABLE ledger_compression_snapshots (
    id UUID PRIMARY KEY,

    batch_id VARCHAR(255),

    original_size_bytes BIGINT,

    compressed_size_bytes BIGINT,

    compression_ratio NUMERIC(18,7),

    created_at TIMESTAMP NOT NULL
);