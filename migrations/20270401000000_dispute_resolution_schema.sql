-- Migration: Merchant Dispute Resolution & Clawback Management (Issue #337)
-- Creates tables for disputes, evidence, and audit trail.

-- ---------------------------------------------------------------------------
-- Custom types
-- ---------------------------------------------------------------------------

CREATE TYPE dispute_status AS ENUM (
    'open',
    'under_review',
    'mediation',
    'resolved_customer',
    'resolved_merchant',
    'resolved_partial',
    'closed'
);

CREATE TYPE dispute_reason AS ENUM (
    'item_not_received',
    'wrong_amount_charged',
    'damaged_goods',
    'unauthorised_charge',
    'service_not_provided',
    'other'
);

CREATE TYPE evidence_submitter AS ENUM (
    'customer',
    'merchant',
    'system'
);

CREATE TYPE dispute_decision AS ENUM (
    'full_refund',
    'partial_refund',
    'no_refund',
    'withdrawn'
);

-- ---------------------------------------------------------------------------
-- disputes
-- ---------------------------------------------------------------------------

CREATE TABLE disputes (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id              UUID        NOT NULL,
    customer_wallet             TEXT        NOT NULL,
    merchant_id                 UUID        NOT NULL,
    reason                      dispute_reason NOT NULL,
    description                 TEXT        NOT NULL,
    status                      dispute_status NOT NULL DEFAULT 'open',

    -- Amounts (stored in cNGN base units)
    transaction_amount          NUMERIC(28, 8) NOT NULL,
    claimed_amount              NUMERIC(28, 8) NOT NULL,
    refunded_amount             NUMERIC(28, 8),

    -- Merchant response window
    merchant_response_deadline  TIMESTAMPTZ NOT NULL,
    merchant_responded_at       TIMESTAMPTZ,
    settlement_proposal         JSONB,

    -- Resolution
    final_decision              dispute_decision,
    refund_tx_hash              TEXT,

    -- Provisional escrow
    escrow_active               BOOLEAN     NOT NULL DEFAULT FALSE,
    escrow_hold_pct             NUMERIC(5, 2),

    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at                 TIMESTAMPTZ
);

-- Indexes for common query patterns
CREATE INDEX idx_disputes_customer_wallet ON disputes (customer_wallet);
CREATE INDEX idx_disputes_merchant_id     ON disputes (merchant_id);
CREATE INDEX idx_disputes_transaction_id  ON disputes (transaction_id);
CREATE INDEX idx_disputes_status          ON disputes (status);
CREATE INDEX idx_disputes_deadline        ON disputes (merchant_response_deadline)
    WHERE status = 'open';

-- ---------------------------------------------------------------------------
-- dispute_evidence
-- ---------------------------------------------------------------------------

CREATE TABLE dispute_evidence (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dispute_id       UUID            NOT NULL REFERENCES disputes (id) ON DELETE CASCADE,
    submitter        evidence_submitter NOT NULL,
    submitter_id     TEXT            NOT NULL,
    label            TEXT            NOT NULL,
    file_url         TEXT,
    notes            TEXT,
    -- Automatically pulled delivery status from shipping provider
    delivery_status  TEXT,
    created_at       TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dispute_evidence_dispute_id ON dispute_evidence (dispute_id);

-- ---------------------------------------------------------------------------
-- dispute_audit_log
-- ---------------------------------------------------------------------------

CREATE TABLE dispute_audit_log (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dispute_id       UUID            NOT NULL REFERENCES disputes (id) ON DELETE CASCADE,
    actor            TEXT            NOT NULL,
    action           TEXT            NOT NULL,
    previous_status  dispute_status,
    new_status       dispute_status,
    notes            TEXT,
    created_at       TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dispute_audit_log_dispute_id ON dispute_audit_log (dispute_id);

-- ---------------------------------------------------------------------------
-- updated_at trigger
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION update_disputes_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_disputes_updated_at
    BEFORE UPDATE ON disputes
    FOR EACH ROW EXECUTE FUNCTION update_disputes_updated_at();
