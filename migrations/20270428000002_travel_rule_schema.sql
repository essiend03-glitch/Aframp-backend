-- Migration: Travel Rule Compliance Schema (Issue #393)
-- VASP-to-VASP PII exchange records and VASP registry

CREATE TYPE travel_rule_status AS ENUM (
    'pending',
    'acknowledged',
    'failed',
    'manual_review',
    'timed_out',
    'completed'
);

CREATE TYPE travel_rule_protocol AS ENUM (
    'trisa',
    'trust',
    'open_vasp',
    'ivms101_direct',
    'unknown'
);

-- Travel Rule exchange records
-- originator_ivms101 and beneficiary_ivms101 are encrypted at rest via application-layer AES-GCM
CREATE TABLE travel_rule_exchanges (
    exchange_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id          TEXT NOT NULL,
    originator_vasp_id      TEXT NOT NULL,
    beneficiary_vasp_id     TEXT NOT NULL,
    protocol_used           travel_rule_protocol NOT NULL DEFAULT 'unknown',
    status                  travel_rule_status NOT NULL DEFAULT 'pending',
    originator_ivms101      JSONB NOT NULL DEFAULT '{}',  -- encrypted PII
    beneficiary_ivms101     JSONB NOT NULL DEFAULT '{}',  -- encrypted PII
    transfer_amount         TEXT NOT NULL,
    asset_code              TEXT NOT NULL,
    handshake_initiated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at         TIMESTAMPTZ,
    timeout_at              TIMESTAMPTZ NOT NULL,
    failure_reason          TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_travel_rule_transaction_id ON travel_rule_exchanges (transaction_id);
CREATE INDEX idx_travel_rule_status ON travel_rule_exchanges (status);
CREATE INDEX idx_travel_rule_timeout ON travel_rule_exchanges (timeout_at) WHERE status = 'pending';

-- VASP registry for counterparty due diligence
CREATE TABLE vasp_registry (
    vasp_id                 TEXT PRIMARY KEY,
    vasp_name               TEXT NOT NULL,
    supported_protocols     TEXT[] NOT NULL DEFAULT '{}',
    travel_rule_endpoint    TEXT,
    is_verified             BOOLEAN NOT NULL DEFAULT FALSE,
    jurisdiction            TEXT NOT NULL,
    last_verified_at        TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_vasp_registry_verified ON vasp_registry (is_verified);
