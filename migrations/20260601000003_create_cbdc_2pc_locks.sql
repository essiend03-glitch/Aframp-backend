-- migrate:up
-- 2PC Lock Manager Persistence (Issue #499)
-- Database-backed Two-Phase Commit lock records for crash recovery.
-- The primary lock state is held in Redis for fast atomic operations;
-- this table provides durability and recovery after worker restarts.

CREATE TABLE IF NOT EXISTS cbdc_2pc_locks (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lock_key            TEXT NOT NULL UNIQUE,
    swap_record_id      UUID NOT NULL REFERENCES cbdc_swap_records(id),
    gateway_id          UUID REFERENCES cbdc_gateways(id),
    lock_state          TEXT NOT NULL CHECK (lock_state IN (
        'preparing', 'prepared', 'committing', 'committed', 'rolling_back', 'rolled_back'
    )),
    lock_holder         TEXT NOT NULL,
    lock_acquired_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lock_expires_at     TIMESTAMPTZ NOT NULL,
    prepared_payload    JSONB,
    commit_payload      JSONB,
    rollback_payload    JSONB,
    node_failure_count  INTEGER NOT NULL DEFAULT 0,
    last_heartbeat_at   TIMESTAMPTZ,
    recovered_at        TIMESTAMPTZ,
    error_detail        TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_2pc_locks_state ON cbdc_2pc_locks(lock_state);
CREATE INDEX IF NOT EXISTS idx_2pc_locks_expiry ON cbdc_2pc_locks(lock_expires_at) WHERE lock_state NOT IN ('committed', 'rolled_back');

COMMENT ON TABLE cbdc_2pc_locks IS 'Persistent Two-Phase Commit lock records for crash recovery and disaster recovery';
COMMENT ON COLUMN cbdc_2pc_locks.lock_state IS 'Current 2PC protocol phase — used to resume interrupted transactions after worker restart';

CREATE TRIGGER update_cbdc_2pc_locks_updated_at
    BEFORE UPDATE ON cbdc_2pc_locks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- migrate:down
DROP TRIGGER IF EXISTS update_cbdc_2pc_locks_updated_at ON cbdc_2pc_locks;
DROP TABLE IF EXISTS cbdc_2pc_locks;
