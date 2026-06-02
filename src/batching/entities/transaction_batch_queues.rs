CREATE TABLE transaction_batch_queues (
    id UUID PRIMARY KEY,

    batch_id VARCHAR(255),

    operation_id VARCHAR(255),

    tenant_id UUID,

    priority INTEGER,

    status VARCHAR(32),

    scheduled_execution TIMESTAMP,

    created_at TIMESTAMP NOT NULL
);