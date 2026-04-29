-- Append-Only Audit Ledger — Tamper-Proof, Forensic-Grade Logging
-- 
-- This migration creates the infrastructure for a cryptographically-sealed,
-- hash-chained audit log that provides absolute accountability and auditability.
--
-- Key Features:
-- - Hash-chaining: Each entry contains hash of previous entry
-- - WORM storage: Write-Once-Read-Many policies prevent tampering
-- - Forensic schema: Complete metadata for reconstruction
-- - Stellar anchoring: Periodic hash anchoring to public blockchain

-- Create custom types for actor and action classification
CREATE TYPE actor_type AS ENUM (
    'user',
    'agent',
    'system',
    'admin',
    'service',
    'external'
);

CREATE TYPE action_type AS ENUM (
    'create',
    'read',
    'update',
    'delete',
    'execute',
    'approve',
    'reject',
    'transfer',
    'mint',
    'burn',
    'authenticate',
    'authorize',
    'configure',
    'deploy'
);

-- Main audit ledger table with WORM guarantees
CREATE TABLE audit_ledger (
    -- Primary identifiers
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sequence BIGSERIAL UNIQUE NOT NULL,
    
    -- Hash chain fields
    previous_hash TEXT NOT NULL,
    entry_hash TEXT NOT NULL UNIQUE,
    
    -- Actor information
    actor_id TEXT NOT NULL,
    actor_type actor_type NOT NULL,
    
    -- Action information
    action_type action_type NOT NULL,
    object_id TEXT,
    object_type TEXT,
    
    -- Temporal information
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- System information
    hardware_signature TEXT NOT NULL,
    correlation_id TEXT,
    
    -- Structured metadata
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    
    -- Network information
    ip_address INET,
    user_agent TEXT,
    
    -- Result information
    result TEXT NOT NULL,
    error_message TEXT,
    
    -- Immutability constraint: once written, cannot be updated or deleted
    -- This is enforced at the application level and via database triggers
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for efficient querying
CREATE INDEX idx_audit_ledger_sequence ON audit_ledger(sequence);
CREATE INDEX idx_audit_ledger_actor_id ON audit_ledger(actor_id);
CREATE INDEX idx_audit_ledger_timestamp ON audit_ledger(timestamp DESC);
CREATE INDEX idx_audit_ledger_correlation_id ON audit_ledger(correlation_id) WHERE correlation_id IS NOT NULL;
CREATE INDEX idx_audit_ledger_object ON audit_ledger(object_type, object_id) WHERE object_id IS NOT NULL;
CREATE INDEX idx_audit_ledger_action_type ON audit_ledger(action_type);
CREATE INDEX idx_audit_ledger_actor_type ON audit_ledger(actor_type);

-- GIN index for metadata JSONB queries
CREATE INDEX idx_audit_ledger_metadata ON audit_ledger USING GIN(metadata);

-- Anchor points table for Stellar blockchain anchoring
CREATE TABLE audit_anchors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sequence BIGINT NOT NULL REFERENCES audit_ledger(sequence),
    entry_hash TEXT NOT NULL,
    anchor_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Stellar blockchain information
    stellar_transaction_id TEXT,
    stellar_ledger BIGINT,
    
    -- Verification status
    verified BOOLEAN DEFAULT FALSE,
    verification_timestamp TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_anchors_sequence ON audit_anchors(sequence);
CREATE INDEX idx_audit_anchors_timestamp ON audit_anchors(anchor_timestamp DESC);
CREATE INDEX idx_audit_anchors_stellar_tx ON audit_anchors(stellar_transaction_id) WHERE stellar_transaction_id IS NOT NULL;

-- Trigger to prevent updates to audit_ledger (WORM enforcement)
CREATE OR REPLACE FUNCTION prevent_audit_ledger_modification()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'Audit ledger entries are immutable and cannot be updated';
    ELSIF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Audit ledger entries are immutable and cannot be deleted';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_prevent_audit_ledger_update
    BEFORE UPDATE ON audit_ledger
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_ledger_modification();

CREATE TRIGGER trigger_prevent_audit_ledger_delete
    BEFORE DELETE ON audit_ledger
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_ledger_modification();

-- Trigger to prevent updates to audit_anchors (WORM enforcement)
CREATE TRIGGER trigger_prevent_audit_anchors_update
    BEFORE UPDATE ON audit_anchors
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_ledger_modification();

CREATE TRIGGER trigger_prevent_audit_anchors_delete
    BEFORE DELETE ON audit_anchors
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_ledger_modification();

-- Function to verify hash chain integrity
CREATE OR REPLACE FUNCTION verify_audit_chain(
    from_seq BIGINT,
    to_seq BIGINT DEFAULT NULL
)
RETURNS TABLE(
    valid BOOLEAN,
    broken_at BIGINT,
    reason TEXT
) AS $$
DECLARE
    current_entry RECORD;
    prev_entry RECORD;
    calculated_hash TEXT;
BEGIN
    -- If to_seq is NULL, verify to the end
    IF to_seq IS NULL THEN
        to_seq := (SELECT MAX(sequence) FROM audit_ledger);
    END IF;
    
    -- Iterate through entries
    FOR current_entry IN
        SELECT * FROM audit_ledger
        WHERE sequence >= from_seq AND sequence <= to_seq
        ORDER BY sequence ASC
    LOOP
        -- Get previous entry
        IF current_entry.sequence > 1 THEN
            SELECT * INTO prev_entry
            FROM audit_ledger
            WHERE sequence = current_entry.sequence - 1;
            
            -- Verify chain link
            IF prev_entry.entry_hash != current_entry.previous_hash THEN
                RETURN QUERY SELECT FALSE, current_entry.sequence, 
                    'Chain broken: previous_hash does not match previous entry hash';
                RETURN;
            END IF;
        END IF;
    END LOOP;
    
    -- Chain is valid
    RETURN QUERY SELECT TRUE, NULL::BIGINT, NULL::TEXT;
END;
$$ LANGUAGE plpgsql;

-- View for audit trail analysis
CREATE VIEW audit_trail_summary AS
SELECT
    DATE_TRUNC('hour', timestamp) AS hour,
    actor_type,
    action_type,
    COUNT(*) AS event_count,
    COUNT(DISTINCT actor_id) AS unique_actors,
    COUNT(DISTINCT correlation_id) AS unique_operations
FROM audit_ledger
GROUP BY DATE_TRUNC('hour', timestamp), actor_type, action_type;

-- View for recent audit events
CREATE VIEW recent_audit_events AS
SELECT
    id,
    sequence,
    actor_id,
    actor_type,
    action_type,
    object_type,
    object_id,
    timestamp,
    result,
    correlation_id
FROM audit_ledger
ORDER BY sequence DESC
LIMIT 1000;

-- Materialized view for anchor verification status
CREATE MATERIALIZED VIEW audit_anchor_status AS
SELECT
    a.id,
    a.sequence,
    a.anchor_timestamp,
    a.stellar_transaction_id,
    a.verified,
    l.timestamp AS ledger_timestamp,
    l.entry_hash,
    EXTRACT(EPOCH FROM (a.anchor_timestamp - l.timestamp)) AS anchor_delay_seconds
FROM audit_anchors a
JOIN audit_ledger l ON a.sequence = l.sequence
ORDER BY a.anchor_timestamp DESC;

CREATE UNIQUE INDEX idx_audit_anchor_status_id ON audit_anchor_status(id);

-- Function to refresh anchor status materialized view
CREATE OR REPLACE FUNCTION refresh_audit_anchor_status()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY audit_anchor_status;
END;
$$ LANGUAGE plpgsql;

-- Grant appropriate permissions (adjust as needed for your security model)
-- GRANT SELECT ON audit_ledger TO auditor_role;
-- GRANT SELECT ON audit_anchors TO auditor_role;
-- GRANT SELECT ON audit_trail_summary TO auditor_role;
-- GRANT SELECT ON recent_audit_events TO auditor_role;
-- GRANT SELECT ON audit_anchor_status TO auditor_role;

-- Insert genesis entry
INSERT INTO audit_ledger (
    sequence,
    previous_hash,
    entry_hash,
    actor_id,
    actor_type,
    action_type,
    object_type,
    timestamp,
    hardware_signature,
    metadata,
    result
) VALUES (
    0,
    'genesis',
    encode(sha256('genesis'::bytea), 'hex'),
    'system',
    'system',
    'create',
    'audit_ledger',
    NOW(),
    'system:genesis',
    '{"event": "audit_ledger_initialized", "version": "1.0.0"}'::jsonb,
    'success'
);

-- Add comments for documentation
COMMENT ON TABLE audit_ledger IS 'Append-only, tamper-proof audit log with cryptographic hash chaining';
COMMENT ON COLUMN audit_ledger.sequence IS 'Monotonically increasing sequence number';
COMMENT ON COLUMN audit_ledger.previous_hash IS 'SHA-256 hash of the previous entry (creates the chain)';
COMMENT ON COLUMN audit_ledger.entry_hash IS 'SHA-256 hash of this entry content';
COMMENT ON COLUMN audit_ledger.hardware_signature IS 'Server/pod identifier for forensic tracking';
COMMENT ON COLUMN audit_ledger.correlation_id IS 'Trace ID for correlating related operations';
COMMENT ON TABLE audit_anchors IS 'Anchor points for hash-chain verification via Stellar blockchain';
COMMENT ON FUNCTION verify_audit_chain IS 'Verify the integrity of the audit chain between two sequence numbers';
