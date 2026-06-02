-- #491 Decentralized Compliance Oracle & Identity Verification Bridge Schemas
-- Tables: compliance_oracles, identity_attestations, compliance_query_logs

-- ── Compliance oracles ────────────────────────────────────────────────────────

CREATE TYPE oracle_status AS ENUM ('active', 'degraded', 'offline');
CREATE TYPE did_method AS ENUM ('did_web', 'did_key', 'did_ethr', 'did_stellar', 'did_ion');

CREATE TABLE compliance_oracles (
    oracle_id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                VARCHAR(120)   NOT NULL,
    endpoint_url        TEXT           NOT NULL,
    status              oracle_status  NOT NULL DEFAULT 'active',
    -- Supported DID methods
    supported_did_methods did_method[] NOT NULL DEFAULT '{}',
    -- Oracle's cryptographic public key (PEM or hex)
    public_key          TEXT           NOT NULL,
    -- Query pricing in USD cents per query
    price_per_query_usd_cents INTEGER   NOT NULL DEFAULT 0,
    -- SLA window in milliseconds
    sla_ms              INTEGER        NOT NULL DEFAULT 150,
    -- Priority order for fallback routing (lower = higher priority)
    priority            INTEGER        NOT NULL DEFAULT 100,
    last_checked_at     TIMESTAMPTZ,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_oracles_status   ON compliance_oracles(status);
CREATE INDEX idx_oracles_priority ON compliance_oracles(priority);

-- ── Identity attestations ─────────────────────────────────────────────────────

CREATE TYPE attestation_status AS ENUM ('valid', 'expired', 'revoked', 'pending');

CREATE TABLE identity_attestations (
    attestation_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Cross-chain address being attested
    cross_chain_address VARCHAR(256)   NOT NULL,
    did_identifier      TEXT           NOT NULL,
    did_method          did_method     NOT NULL,
    oracle_id           UUID           NOT NULL REFERENCES compliance_oracles(oracle_id),
    -- Cryptographic proof hash (no raw PII stored)
    proof_hash          VARCHAR(128)   NOT NULL,
    -- ZK-proof certificate reference (opaque identifier)
    zk_proof_ref        VARCHAR(256),
    -- Issuer signature (hex-encoded)
    issuer_signature    TEXT           NOT NULL,
    status              attestation_status NOT NULL DEFAULT 'valid',
    -- Verification markers (no PII — only boolean flags)
    is_sanctions_clear  BOOLEAN        NOT NULL DEFAULT FALSE,
    is_kyc_verified     BOOLEAN        NOT NULL DEFAULT FALSE,
    is_aml_clear        BOOLEAN        NOT NULL DEFAULT FALSE,
    issued_at           TIMESTAMPTZ    NOT NULL,
    expires_at          TIMESTAMPTZ    NOT NULL,
    revoked_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_attestation_address_did
    ON identity_attestations(cross_chain_address, did_identifier)
    WHERE status = 'valid';
CREATE INDEX idx_attestation_address ON identity_attestations(cross_chain_address);
CREATE INDEX idx_attestation_expires  ON identity_attestations(expires_at)
    WHERE status = 'valid';

-- ── Compliance query logs ─────────────────────────────────────────────────────

CREATE TYPE query_outcome AS ENUM (
    'cleared', 'blocked', 'amber_review', 'error', 'cache_hit', 'timed_out'
);

CREATE TABLE compliance_query_logs (
    query_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Originating cross-chain transaction ID
    originating_tx_id   UUID           NOT NULL,
    cross_chain_address VARCHAR(256)   NOT NULL,
    oracle_id           UUID           REFERENCES compliance_oracles(oracle_id),
    outcome             query_outcome  NOT NULL,
    -- Latency in milliseconds
    latency_ms          INTEGER,
    -- Whether result was served from Redis cache
    cache_hit           BOOLEAN        NOT NULL DEFAULT FALSE,
    -- Cryptographic signature envelope reference
    signature_envelope  TEXT,
    error_detail        TEXT,
    queried_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_query_logs_tx      ON compliance_query_logs(originating_tx_id);
CREATE INDEX idx_query_logs_address ON compliance_query_logs(cross_chain_address);
CREATE INDEX idx_query_logs_outcome ON compliance_query_logs(outcome);
CREATE INDEX idx_query_logs_time    ON compliance_query_logs(queried_at DESC);
