CREATE TABLE operation_fee_allocations (
    id UUID PRIMARY KEY,

    batch_id VARCHAR(255),

    operation_id VARCHAR(255),

    tenant_id UUID,

    allocated_fee NUMERIC(30,7),

    fee_weight NUMERIC(30,7),

    created_at TIMESTAMP NOT NULL
);