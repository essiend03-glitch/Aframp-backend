-- Mint Authorization Framework (#213)
-- Governs cNGN issuance via multi-signature approval before Stellar submission.

-- ─────────────────────────────────────────────────────────────────────────────
-- Types
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TYPE mint_auth_status AS ENUM (
    'pending_signatures',
    'threshold_met',
    'submitted',
    'confirmed',
    'failed',
    'expired',
    'cancelled'
);

-- ─────────────────────────────────────────────────────────────────────────────
-- Core tables
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE mint_authorization_requests (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Mint details
    amount_cngn             NUMERIC(36, 7) NOT NULL CHECK (amount_cngn > 0),
    destination_account     VARCHAR(64) NOT NULL,   -- Stellar distribution account
    requested_by            UUID NOT NULL,           -- admin user id
    requested_by_key        VARCHAR(64) NOT NULL,    -- Stellar public key of requester
    justification           TEXT NOT NULL,

    -- Reserve verification link
    reserve_verification_id UUID NOT NULL,           -- FK to historical_verification

    -- Signature collection
    required_signatures     SMALLINT NOT NULL CHECK (required_signatures > 0),
    collected_signatures    SMALLINT NOT NULL DEFAULT 0,

    -- Transaction envelope
    unsigned_xdr            TEXT NOT NULL,           -- base64 XDR, no signatures
    signed_xdr              TEXT,                    -- base64 XDR, fully signed
    tx_hash                 TEXT,                    -- SHA-256 hash signers must sign
    stellar_tx_hash         TEXT,                    -- hash returned by Horizon on submission

    -- Lifecycle
    status                  mint_auth_status NOT NULL DEFAULT 'pending_signatures',
    failure_reason          TEXT,
    cancellation_reason     TEXT,
    cancelled_by            UUID,
    retry_count             SMALLINT NOT NULL DEFAULT 0,

    expires_at              TIMESTAMPTZ NOT NULL,
    submitted_at            TIMESTAMPTZ,
    confirmed_at            TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE mint_authorization_signatures (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    auth_request_id UUID NOT NULL REFERENCES mint_authorization_requests(id) ON DELETE CASCADE,
    signer_id       UUID NOT NULL REFERENCES mint_signers(id),
    signer_key      VARCHAR(64) NOT NULL,   -- Stellar public key (G…)
    -- Raw 64-byte Ed25519 signature over tx_hash, base64-encoded
    signature       TEXT NOT NULL,
    signed_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ip_address      INET,

    UNIQUE (auth_request_id, signer_id)
);

-- ─────────────────────────────────────────────────────────────────────────────
-- Indexes
-- ─────────────────────────────────────────────────────────────────────────────

CREATE INDEX ON mint_authorization_requests (status, created_at DESC);
CREATE INDEX ON mint_authorization_requests (expires_at) WHERE status = 'pending_signatures';
CREATE INDEX ON mint_authorization_requests (reserve_verification_id);
CREATE INDEX ON mint_authorization_signatures (auth_request_id);
CREATE INDEX ON mint_authorization_signatures (signer_id);

-- ─────────────────────────────────────────────────────────────────────────────
-- updated_at trigger
-- ─────────────────────────────────────────────────────────────────────────────

CREATE OR REPLACE FUNCTION mint_auth_set_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$;

CREATE TRIGGER trg_mint_auth_requests_updated_at
    BEFORE UPDATE ON mint_authorization_requests
    FOR EACH ROW EXECUTE FUNCTION mint_auth_set_updated_at();
