-- migrate:up
-- CBDC Cross-Rail Swap Records (Issue #499)
-- Immutable audit trail mapping on-chain Stellar transaction hashes to
-- corresponding central bank ledger block IDs for atomic cross-rail swaps.

CREATE TABLE IF NOT EXISTS cbdc_swap_records (
    id                              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swap_type                       TEXT NOT NULL CHECK (swap_type IN ('mint', 'burn', 'cross_rail_settlement')),
    status                          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending', 'prepared', 'committed_cbdc', 'committed_stellar',
        'completed', 'failed', 'held_for_reconciliation', 'reversed'
    )),

    -- Stellar leg
    stellar_transaction_hash        TEXT,
    stellar_asset_code              TEXT NOT NULL,
    stellar_asset_issuer            TEXT,
    stellar_amount                  NUMERIC(36, 18) NOT NULL,
    stellar_source_account          TEXT,
    stellar_destination_account     TEXT,
    stellar_trustline               TEXT,
    stellar_sequence_number         BIGINT,
    stellar_ledger                  BIGINT,

    -- CBDC leg
    cbdc_gateway_id                 UUID REFERENCES cbdc_gateways(id),
    cbdc_transaction_id             TEXT,
    cbdc_block_id                   TEXT,
    cbdc_block_number               BIGINT,
    cbdc_confirmations              INTEGER DEFAULT 0,
    cbdc_sender                     TEXT,
    cbdc_recipient                  TEXT,
    cbdc_amount                     NUMERIC(36, 18) NOT NULL,
    cbdc_currency                   TEXT NOT NULL,
    cbdc_raw_payload                JSONB,

    -- 2PC state
    two_phase_state                 TEXT NOT NULL DEFAULT 'none' CHECK (two_phase_state IN (
        'none', 'preparing', 'prepared', 'committing', 'committed', 'rolling_back', 'rolled_back'
    )),
    two_phase_lock_id               TEXT,
    two_phase_prepared_at           TIMESTAMPTZ,
    two_phase_committed_at          TIMESTAMPTZ,

    -- AML / Compliance
    aml_screening_id                TEXT,
    aml_screening_result            TEXT CHECK (aml_screening_result IN ('pass', 'fail', 'pending', 'escalated')),
    compliance_metadata             JSONB DEFAULT '{}'::jsonb,

    -- Settlement worker tracking
    worker_id                       TEXT,
    worker_attempts                 INTEGER NOT NULL DEFAULT 0,
    worker_last_error               TEXT,
    worker_scheduled_at             TIMESTAMPTZ,
    worker_completed_at             TIMESTAMPTZ,

    -- Multi-sig signatory approvals
    required_approvals              INTEGER NOT NULL DEFAULT 1,
    current_approvals               INTEGER NOT NULL DEFAULT 0,
    approval_threshold_met          BOOLEAN NOT NULL DEFAULT FALSE,

    -- Audit trail
    error_message                   TEXT,
    error_code                      TEXT,
    idempotency_key                 TEXT UNIQUE NOT NULL,
    reversal_of                     UUID REFERENCES cbdc_swap_records(id),
    created_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cbdc_swap_status ON cbdc_swap_records(status);
CREATE INDEX IF NOT EXISTS idx_cbdc_swap_stellar_hash ON cbdc_swap_records(stellar_transaction_hash);
CREATE INDEX IF NOT EXISTS idx_cbdc_swap_cbdc_tx ON cbdc_swap_records(cbdc_transaction_id);
CREATE INDEX IF NOT EXISTS idx_cbdc_swap_gateway ON cbdc_swap_records(cbdc_gateway_id);
CREATE INDEX IF NOT EXISTS idx_cbdc_swap_two_phase ON cbdc_swap_records(two_phase_state) WHERE two_phase_state != 'none';
CREATE INDEX IF NOT EXISTS idx_cbdc_swap_idempotency ON cbdc_swap_records(idempotency_key);
CREATE INDEX IF NOT EXISTS idx_cbdc_swap_created ON cbdc_swap_records(created_at);

COMMENT ON TABLE cbdc_swap_records IS 'Immutable cross-rail swap audit trail linking Stellar transactions to CBDC ledger operations';
COMMENT ON COLUMN cbdc_swap_records.two_phase_state IS 'Current state within the Two-Phase Commit (2PC) protocol lifecycle';
COMMENT ON COLUMN cbdc_swap_records.reversal_of IS 'References the original swap record if this entry is a reversal';
COMMENT ON COLUMN cbdc_swap_records.idempotency_key IS 'Unique idempotency key to guarantee exactly-once processing of swap requests';

CREATE TRIGGER update_cbdc_swap_records_updated_at
    BEFORE UPDATE ON cbdc_swap_records
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- migrate:down
DROP TRIGGER IF EXISTS update_cbdc_swap_records_updated_at ON cbdc_swap_records;
DROP TABLE IF EXISTS cbdc_swap_records;
