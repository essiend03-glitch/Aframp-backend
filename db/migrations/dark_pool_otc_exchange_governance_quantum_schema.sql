-- ============================================================================
-- Institutional Dark Pool, OTC Aggregator, Governance, and Quantum Key Registry
-- ============================================================================

CREATE TYPE IF NOT EXISTS counterparty_status AS ENUM ('active', 'suspended', 'revoked');

CREATE TABLE IF NOT EXISTS dark_pool_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id UUID NOT NULL,
    order_side VARCHAR(4) NOT NULL CHECK (order_side IN ('buy','sell')),
    asset_pair VARCHAR(32) NOT NULL,
    min_size DECIMAL(28,7) NOT NULL,
    max_size DECIMAL(28,7) NOT NULL,
    limit_price DECIMAL(28,7) NOT NULL,
    max_slippage_bps INTEGER NOT NULL DEFAULT 50,
    priority_ts TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    masking_id UUID NOT NULL DEFAULT gen_random_uuid(),
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_dark_pool_orders_asset_pair_status ON dark_pool_orders(asset_pair, status);
CREATE INDEX IF NOT EXISTS idx_dark_pool_orders_priority_ts ON dark_pool_orders(priority_ts DESC);

CREATE TABLE IF NOT EXISTS otc_counterparty_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    legal_name TEXT NOT NULL,
    institution_type VARCHAR(50),
    verification_status counterparty_status NOT NULL DEFAULT 'active',
    credit_facilities JSONB NOT NULL DEFAULT '{}'::jsonb,
    settlement_sla TEXT,
    crypto_id JSONB,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_otc_counterparty_profiles_verification_status ON otc_counterparty_profiles(verification_status);

CREATE TABLE IF NOT EXISTS matched_block_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_window TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    block_reference UUID NOT NULL,
    buy_order_id UUID NOT NULL REFERENCES dark_pool_orders(id) ON DELETE SET NULL,
    sell_order_id UUID NOT NULL REFERENCES dark_pool_orders(id) ON DELETE SET NULL,
    execution_price DECIMAL(28,7) NOT NULL,
    execution_size DECIMAL(28,7) NOT NULL,
    internal_sequence BIGSERIAL,
    public_ledger_id UUID,
    status VARCHAR(32) NOT NULL DEFAULT 'matched',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
) PARTITION BY RANGE (created_at);

CREATE TABLE IF NOT EXISTS matched_block_executions_2026_q3 PARTITION OF matched_block_executions FOR VALUES FROM ('2026-07-01') TO ('2026-10-01');

CREATE TABLE IF NOT EXISTS exchange_venues (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    venue_name TEXT NOT NULL,
    api_credentials JSONB NOT NULL DEFAULT '{}'::jsonb,
    websocket_endpoint TEXT,
    rest_endpoint TEXT,
    fee_schedule JSONB NOT NULL DEFAULT '{}'::jsonb,
    min_volume DECIMAL(28,7),
    max_volume DECIMAL(28,7),
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS aggregated_order_books (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    venue_id UUID REFERENCES exchange_venues(id) ON DELETE SET NULL,
    currency_pair VARCHAR(32) NOT NULL,
    depth_snapshot JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_aggregated_order_books_pair ON aggregated_order_books(currency_pair, updated_at DESC);

CREATE TABLE IF NOT EXISTS smart_split_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_order_id UUID NOT NULL,
    currency_pair VARCHAR(32) NOT NULL,
    total_volume DECIMAL(28,7) NOT NULL,
    split_path JSONB NOT NULL,
    venue_allocations JSONB NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TYPE IF NOT EXISTS governance_proposal_state AS ENUM ('PROPOSED','VOTING','APPROVED','TIME_LOCKED','EXECUTED','REJECTED');

CREATE TABLE IF NOT EXISTS governance_proposals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    description TEXT,
    target_payload JSONB,
    status governance_proposal_state NOT NULL DEFAULT 'PROPOSED',
    proposer_public_key TEXT NOT NULL,
    time_lock_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS proposal_ballots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES governance_proposals(id) ON DELETE CASCADE,
    voter_public_key TEXT NOT NULL,
    vote_weight NUMERIC(18,7) NOT NULL DEFAULT 1.0,
    signature TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS timelock_execution_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID NOT NULL REFERENCES governance_proposals(id) ON DELETE CASCADE,
    execute_after TIMESTAMPTZ NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    payload JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
) PARTITION BY RANGE (execute_after);

CREATE TABLE IF NOT EXISTS timelock_execution_queue_2026_q3 PARTITION OF timelock_execution_queue FOR VALUES FROM ('2026-07-01') TO ('2026-10-01');

CREATE TYPE IF NOT EXISTS pqc_algorithm AS ENUM ('ML-DSA-65','XMSS');

CREATE TABLE IF NOT EXISTS quantum_secure_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_alias TEXT NOT NULL,
    algorithm pqc_algorithm NOT NULL,
    public_key TEXT NOT NULL,
    derivation_path TEXT,
    vault_address TEXT,
    operational_state VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS x509_pqc_certificates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    certificate_pem TEXT NOT NULL,
    subject TEXT NOT NULL,
    algorithm pqc_algorithm NOT NULL,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS hybrid_signature_audit_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id UUID NOT NULL,
    classical_sig TEXT NOT NULL,
    pqc_sig TEXT NOT NULL,
    signer_public_key TEXT NOT NULL,
    proof_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
) PARTITION BY RANGE (created_at);

CREATE TABLE IF NOT EXISTS hybrid_signature_audit_ledger_2026_q3 PARTITION OF hybrid_signature_audit_ledger FOR VALUES FROM ('2026-07-01') TO ('2026-10-01');
