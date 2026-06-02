-- Bank Integrations Schema (Issue #407 Extended)
-- Corporate banking partners, virtual accounts, and settlement tracking

-- Bank Integrations: Track active corporate banking partners
CREATE TABLE bank_integrations (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_name            TEXT NOT NULL UNIQUE,
    partner_code            TEXT NOT NULL UNIQUE,
    api_base_url            TEXT NOT NULL,
    api_key_secret_ref      TEXT NOT NULL, -- Reference to secret store
    webhook_secret_ref      TEXT NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'active', -- 'active', 'inactive', 'suspended'
    settlement_pool_account TEXT,
    settlement_bank_code    TEXT,
    settlement_account_name TEXT,
    settlement_account_number TEXT,
    priority_weight         INTEGER DEFAULT 1,
    rate_limit_rpm          INTEGER DEFAULT 60,
    timeout_seconds         INTEGER DEFAULT 30,
    health_check_url        TEXT,
    last_health_check       TIMESTAMPTZ,
    last_health_status      TEXT,
    webhook_backlog_count   INTEGER DEFAULT 0,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bank_integrations_status ON bank_integrations (status);
CREATE INDEX idx_bank_integrations_code ON bank_integrations (partner_code);

-- Virtual Accounts: Track generated dedicated inbound payment accounts
CREATE TABLE virtual_accounts (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES kyc_records(consumer_id),
    bank_integration_id     UUID REFERENCES bank_integrations(id),
    virtual_account_number  TEXT NOT NULL UNIQUE,
    virtual_account_name    TEXT NOT NULL,
    bank_code               TEXT NOT NULL,
    bank_name               TEXT NOT NULL,
    assignment_state        TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'active', 'suspended', 'closed'
    settlement_tracking_code TEXT,
    expected_amount         DECIMAL(20, 2),
    expected_currency       TEXT DEFAULT 'NGN',
    settlement_reference    TEXT,
    settled_amount          DECIMAL(20, 2) DEFAULT 0,
    settled_at              TIMESTAMPTZ,
    last_transaction_at     TIMESTAMPTZ,
    is_primary              BOOLEAN DEFAULT FALSE,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_virtual_accounts_user ON virtual_accounts (user_id);
CREATE INDEX idx_virtual_accounts_number ON virtual_accounts (virtual_account_number);
CREATE INDEX idx_virtual_accounts_state ON virtual_accounts (assignment_state);
CREATE INDEX idx_virtual_accounts_tracking ON virtual_accounts (settlement_tracking_code);

-- Fiat Settlements: Track fiat deposits and cNGN minting
CREATE TABLE fiat_settlements (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    virtual_account_id      UUID REFERENCES virtual_accounts(id),
    user_id                 UUID NOT NULL REFERENCES kyc_records(consumer_id),
    bank_integration_id     UUID REFERENCES bank_integrations(id),
    bank_transaction_id     TEXT NOT NULL,
    bank_reference          TEXT,
    amount                  DECIMAL(20, 2) NOT NULL,
    currency                TEXT NOT NULL DEFAULT 'NGN',
    cn gn_amount            DECIMAL(20, 8),
    cn gn_minted            BOOLEAN DEFAULT FALSE,
    wallet_address          TEXT,
    settlement_status       TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'confirmed', 'minting', 'completed', 'failed'
    settlement_error        TEXT,
    webhook_event_id        UUID,
    confirmed_at            TIMESTAMPTZ,
    minted_at               TIMESTAMPTZ,
    completed_at            TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fiat_settlements_user ON fiat_settlements (user_id);
CREATE INDEX idx_fiat_settlements_virtual ON fiat_settlements (virtual_account_id);
CREATE INDEX idx_fiat_settlements_status ON fiat_settlements (settlement_status);
CREATE INDEX idx_fiat_settlements_bank_txn ON fiat_settlements (bank_transaction_id);

-- Bank Webhooks: Enhanced webhook tracking with signature validation
CREATE TABLE bank_webhooks (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bank_integration_id     UUID REFERENCES bank_integrations(id),
    event_type              TEXT NOT NULL,
    provider_event_id       TEXT NOT NULL,
    payload                 JSONB NOT NULL,
    signature_valid         BOOLEAN,
    signature_algorithm     TEXT,
    idempotency_key         TEXT UNIQUE,
    processing_status       TEXT NOT NULL DEFAULT 'received', -- 'received', 'processing', 'processed', 'failed', 'duplicate'
    error_message           TEXT,
    retry_count             INTEGER DEFAULT 0,
    processed_at            TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bank_webhooks_integration ON bank_webhooks (bank_integration_id, created_at DESC);
CREATE INDEX idx_bank_webhooks_idempotency ON bank_webhooks (idempotency_key);
CREATE INDEX idx_bank_webhooks_provider_id ON bank_webhooks (provider_event_id, bank_integration_id);

-- Bank API Metrics: Track API latency and performance
CREATE TABLE bank_api_metrics (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bank_integration_id     UUID REFERENCES bank_integrations(id),
    api_endpoint            TEXT NOT NULL,
    method                  TEXT NOT NULL,
    latency_ms              INTEGER NOT NULL,
    status_code             INTEGER,
    error_code              TEXT,
    request_id              TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bank_api_metrics_integration ON bank_api_metrics (bank_integration_id, created_at DESC);
CREATE INDEX idx_bank_api_metrics_latency ON bank_api_metrics (api_endpoint, latency_ms);

-- Reconciliation Jobs: Track manual reconciliation triggers
CREATE TABLE bank_reconciliation_jobs (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bank_integration_id     UUID REFERENCES bank_integrations(id),
    trigger_type            TEXT NOT NULL, -- 'scheduled', 'manual', 'automatic'
    triggered_by            UUID,
    status                  TEXT NOT NULL DEFAULT 'running', -- 'running', 'completed', 'failed'
    records_checked         INTEGER DEFAULT 0,
    discrepancies_found     INTEGER DEFAULT 0,
    started_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at            TIMESTAMPTZ,
    error_message           TEXT,
    metadata                JSONB DEFAULT '{}'
);

CREATE INDEX idx_reconciliation_jobs_status ON bank_reconciliation_jobs (status);
CREATE INDEX idx_reconciliation_jobs_integration ON bank_reconciliation_jobs (bank_integration_id);

-- Add columns to existing linked_bank_accounts if needed
ALTER TABLE linked_bank_accounts 
    ADD COLUMN IF NOT EXISTS bank_integration_id UUID REFERENCES bank_integrations(id),
    ADD COLUMN IF NOT EXISTS is_virtual_account BOOLEAN DEFAULT FALSE;