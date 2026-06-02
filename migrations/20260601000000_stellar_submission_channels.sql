-- Stellar Submission Channels Infrastructure
-- Manages a pool of channel accounts for high-throughput, parallelized transaction submission.
-- Each channel account has its own sequence number management and can be rotated independently.

CREATE TABLE IF NOT EXISTS stellar_submission_channels (
    id                          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_id                   UUID        NOT NULL REFERENCES stellar_issuer_accounts(id) ON DELETE CASCADE,
    environment                 TEXT        NOT NULL CHECK (environment IN ('testnet', 'mainnet')),
    channel_account_id          TEXT        NOT NULL UNIQUE,          -- Stellar public key (G...)
    channel_index               INTEGER     NOT NULL,                 -- 0, 1, 2, ... for channel rotation
    secrets_ref                 TEXT        NOT NULL,                 -- secrets manager key name
    
    -- Sequence Number Management
    current_sequence            BIGINT      NOT NULL DEFAULT 0,       -- Last submitted sequence number
    reserved_sequence           BIGINT      NOT NULL DEFAULT 0,       -- Reserved for in-flight txns
    
    -- Balance & Capacity
    balance_xlm                 NUMERIC     NOT NULL DEFAULT 0.0,     -- XLM balance (native asset)
    min_balance_threshold       NUMERIC     NOT NULL DEFAULT 2.0,     -- XLM (base reserve)
    is_active                   BOOLEAN     NOT NULL DEFAULT true,
    
    -- Submission Statistics
    total_submitted             BIGINT      NOT NULL DEFAULT 0,       -- Lifetime submission count
    total_successful            BIGINT      NOT NULL DEFAULT 0,       -- Confirmed on-chain
    total_failed                BIGINT      NOT NULL DEFAULT 0,       -- Failed submissions
    consecutive_failures        INTEGER     NOT NULL DEFAULT 0,       -- For circuit breaker
    last_error_code             TEXT,                                  -- Last Horizon error
    last_error_at               TIMESTAMPTZ,
    
    -- Operational Flags
    in_rotation                 BOOLEAN     NOT NULL DEFAULT true,    -- Include in active pool
    exhaustion_alert_sent_at    TIMESTAMPTZ,                          -- Last <30% capacity alert
    
    -- Metadata
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    CONSTRAINT check_sequence_order CHECK (current_sequence <= reserved_sequence)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_submission_channels_unique
    ON stellar_submission_channels (issuer_id, channel_index);

CREATE INDEX IF NOT EXISTS idx_submission_channels_active
    ON stellar_submission_channels (issuer_id, is_active, in_rotation)
    WHERE is_active = true AND in_rotation = true;

CREATE INDEX IF NOT EXISTS idx_submission_channels_balance_check
    ON stellar_submission_channels (issuer_id, balance_xlm)
    WHERE balance_xlm < 10.0;  -- Low balance alerting

COMMENT ON TABLE stellar_submission_channels IS
    'Channel accounts for parallelized Stellar transaction submission. Each channel manages its own sequence numbers.';
COMMENT ON COLUMN stellar_submission_channels.reserved_sequence IS
    'Sequence number reserved for in-flight txns. current_sequence <= reserved_sequence <= Horizon sequence.';
COMMENT ON COLUMN stellar_submission_channels.consecutive_failures IS
    'Circuit breaker: rotate channel if failures exceed threshold.';

-- ============================================================================
-- Stellar Transaction Logs
-- ============================================================================
-- Records every transaction submitted, including fee, sequence, Horizon hash, and settlement status.
-- Immutable audit trail linked to Stellar ledger hashes.

CREATE TABLE IF NOT EXISTS stellar_transaction_logs (
    id                          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_id                   UUID        NOT NULL REFERENCES stellar_issuer_accounts(id) ON DELETE CASCADE,
    channel_id                  UUID        NOT NULL REFERENCES stellar_submission_channels(id) ON DELETE CASCADE,
    
    -- Submission Metadata
    submission_index            BIGINT      NOT NULL,                -- Sequence number used
    sequence_reserved_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    -- Transaction Details
    tx_envelope_hash            TEXT        NOT NULL,                -- XDR-computed hash before submission
    tx_envelope_xdr             TEXT        NOT NULL,                -- Full XDR (immutable snapshot)
    submission_fee_stroops      BIGINT      NOT NULL,                -- Fee paid (in stroops)
    surge_fee_percent           NUMERIC     NOT NULL DEFAULT 100,    -- Fee multiplier vs base
    
    -- Horizon Submission
    submission_attempt          INTEGER     NOT NULL DEFAULT 1,
    submitted_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    -- Settlement & Confirmation
    confirmed_at                TIMESTAMPTZ,
    stellar_ledger_hash         TEXT,                                -- From Horizon /transactions/{hash}
    stellar_ledger_number       BIGINT,                              -- Ledger sequence containing tx
    stellar_tx_hash             TEXT UNIQUE,                         -- Immutable on-chain hash
    
    -- Error Tracking
    last_error_code             TEXT,                                -- Horizon error code
    last_error_reason           TEXT,                                -- Human-readable error
    last_error_at               TIMESTAMPTZ,
    
    -- Retry State Machine
    retry_count                 INTEGER     NOT NULL DEFAULT 0,
    next_retry_at               TIMESTAMPTZ,
    final_status                TEXT        CHECK (final_status IN ('confirmed', 'failed', 'stale')),
    failure_reason              TEXT,
    
    -- Audit Trail
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_stellar_tx_logs_envelope
    ON stellar_transaction_logs (tx_envelope_hash);

CREATE INDEX IF NOT EXISTS idx_stellar_tx_logs_channel_tracking
    ON stellar_transaction_logs (channel_id, submitted_at DESC)
    WHERE final_status IS NULL;

CREATE INDEX IF NOT EXISTS idx_stellar_tx_logs_confirmation
    ON stellar_transaction_logs (confirmed_at DESC NULLS LAST)
    WHERE confirmed_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_stellar_tx_logs_pending_retry
    ON stellar_transaction_logs (next_retry_at)
    WHERE final_status IS NULL AND retry_count < 5;

CREATE INDEX IF NOT EXISTS idx_stellar_tx_logs_stellar_hash
    ON stellar_transaction_logs (stellar_tx_hash)
    WHERE stellar_tx_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_stellar_tx_logs_ledger_settlement
    ON stellar_transaction_logs (stellar_ledger_number DESC NULLS LAST)
    WHERE stellar_ledger_number IS NOT NULL;

COMMENT ON TABLE stellar_transaction_logs IS
    'Immutable audit trail of all Stellar submissions. One entry per transaction envelope.';
COMMENT ON COLUMN stellar_transaction_logs.tx_envelope_hash IS
    'Hash of the XDR envelope before submission. Used for idempotency checks.';
COMMENT ON COLUMN stellar_transaction_logs.stellar_tx_hash IS
    'Immutable Stellar ledger transaction hash. Populated after on-chain confirmation.';
COMMENT ON COLUMN stellar_transaction_logs.stellar_ledger_number IS
    'Ledger sequence number containing the confirmed transaction.';

-- ============================================================================
-- Channel Exhaustion Alerts
-- ============================================================================
-- Records when a channel drops below 30% capacity (exhaustion), for alerting.

CREATE TABLE IF NOT EXISTS stellar_channel_exhaustion_events (
    id                          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id                  UUID        NOT NULL REFERENCES stellar_submission_channels(id) ON DELETE CASCADE,
    available_slots             INTEGER     NOT NULL,
    total_slots                 INTEGER     NOT NULL,
    utilization_percent         NUMERIC     NOT NULL,
    alert_sent_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_channel_exhaustion_events_recent
    ON stellar_channel_exhaustion_events (channel_id, alert_sent_at DESC);

COMMENT ON TABLE stellar_channel_exhaustion_events IS
    'Audit trail of channel exhaustion warnings when available capacity drops below 30%.';

-- ============================================================================
-- Confirmation Delay Alerts
-- ============================================================================
-- Triggers when a transaction takes > 3 ledgers (15s) to confirm.

CREATE TABLE IF NOT EXISTS stellar_confirmation_delay_alerts (
    id                          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tx_log_id                   UUID        NOT NULL REFERENCES stellar_transaction_logs(id) ON DELETE CASCADE,
    submitted_at                TIMESTAMPTZ NOT NULL,
    ledgers_to_confirm          INTEGER     NOT NULL,
    confirmation_time_seconds   NUMERIC     NOT NULL,
    alert_sent_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_confirmation_delay_alerts_recent
    ON stellar_confirmation_delay_alerts (alert_sent_at DESC);

COMMENT ON TABLE stellar_confirmation_delay_alerts IS
    'Audit trail of confirmation delays exceeding 3 ledgers / 15 seconds.';

-- ============================================================================
-- Down Migration
-- ============================================================================

-- migrate:down

DROP TABLE IF EXISTS stellar_confirmation_delay_alerts CASCADE;
DROP TABLE IF EXISTS stellar_channel_exhaustion_events CASCADE;
DROP TABLE IF EXISTS stellar_transaction_logs CASCADE;
DROP TABLE IF EXISTS stellar_submission_channels CASCADE;
