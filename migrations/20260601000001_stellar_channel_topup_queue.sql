-- Channel Top-Up Queue for operator-initiated balance replenishment
CREATE TABLE IF NOT EXISTS stellar_channel_topup_queue (
    id                          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_index               INTEGER     NOT NULL,
    amount_xlm                  NUMERIC     NOT NULL CHECK (amount_xlm > 0),
    description                 TEXT,
    status                      TEXT        NOT NULL CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    submitted_tx_hash           TEXT,
    completed_at                TIMESTAMPTZ,
    error_reason                TEXT,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_channel_topup_status
    ON stellar_channel_topup_queue (status)
    WHERE status IN ('pending', 'processing');

CREATE INDEX IF NOT EXISTS idx_channel_topup_channel
    ON stellar_channel_topup_queue (channel_index);

COMMENT ON TABLE stellar_channel_topup_queue IS
    'Queue of top-up operations for channel accounts to prevent balance depletion.';

-- migrate:down
DROP TABLE IF EXISTS stellar_channel_topup_queue CASCADE;
