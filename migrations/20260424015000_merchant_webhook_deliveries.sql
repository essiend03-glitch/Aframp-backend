-- Merchant webhook delivery queue with retry/dead-letter support
CREATE TABLE IF NOT EXISTS merchant_webhook_deliveries (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_intent_id   UUID NOT NULL REFERENCES merchant_payment_intents(id) ON DELETE CASCADE,
    merchant_id         UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    webhook_url         TEXT NOT NULL,
    event_type          TEXT NOT NULL,
    payload             JSONB NOT NULL DEFAULT '{}',
    signature           TEXT NOT NULL DEFAULT '',
    idempotency_key     TEXT NOT NULL UNIQUE,
    queue_name          TEXT NOT NULL DEFAULT 'default',
    status              TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending', 'retrying', 'delivered', 'failed', 'dead_lettered')),
    http_status_code    INT,
    response_body       TEXT,
    error_message       TEXT,
    retry_count         INT NOT NULL DEFAULT 0,
    next_retry_at       TIMESTAMPTZ,
    locked_at           TIMESTAMPTZ,
    locked_by           TEXT,
    last_attempt_at     TIMESTAMPTZ,
    delivered_at        TIMESTAMPTZ,
    dead_lettered_at    TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mwd_merchant_id        ON merchant_webhook_deliveries(merchant_id);
CREATE INDEX IF NOT EXISTS idx_mwd_payment_intent_id  ON merchant_webhook_deliveries(payment_intent_id);
CREATE INDEX IF NOT EXISTS idx_mwd_status             ON merchant_webhook_deliveries(status);
CREATE INDEX IF NOT EXISTS idx_mwd_next_retry_at      ON merchant_webhook_deliveries(next_retry_at) WHERE status IN ('pending','retrying');
CREATE INDEX IF NOT EXISTS idx_mwd_created_at         ON merchant_webhook_deliveries(created_at DESC);
