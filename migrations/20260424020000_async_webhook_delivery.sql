-- ============================================================================
-- Async Webhook Delivery & Retry Queues
-- Issue #346
-- ============================================================================
-- Adds explicit queue metadata, deterministic idempotency keys, dead-letter
-- capture, and endpoint circuit breakers to merchant webhook delivery.
-- ============================================================================

ALTER TABLE merchant_webhook_deliveries
    ADD COLUMN IF NOT EXISTS idempotency_key TEXT,
    ADD COLUMN IF NOT EXISTS queue_name TEXT NOT NULL DEFAULT 'primary',
    ADD COLUMN IF NOT EXISTS locked_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS locked_by TEXT,
    ADD COLUMN IF NOT EXISTS dead_lettered_at TIMESTAMPTZ;

UPDATE merchant_webhook_deliveries
SET idempotency_key = CONCAT('legacy-webhook:', id::text)
WHERE idempotency_key IS NULL;

ALTER TABLE merchant_webhook_deliveries
    ALTER COLUMN idempotency_key SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_merchant_webhook_deliveries_idempotency
    ON merchant_webhook_deliveries (idempotency_key);

ALTER TABLE merchant_webhook_deliveries
    DROP CONSTRAINT IF EXISTS merchant_webhook_deliveries_status_check;

ALTER TABLE merchant_webhook_deliveries
    ADD CONSTRAINT merchant_webhook_deliveries_status_check
    CHECK (status IN ('pending', 'retrying', 'delivered', 'failed', 'abandoned', 'dead_lettered'));

ALTER TABLE merchant_webhook_deliveries
    DROP CONSTRAINT IF EXISTS merchant_webhook_deliveries_queue_name_check;

ALTER TABLE merchant_webhook_deliveries
    ADD CONSTRAINT merchant_webhook_deliveries_queue_name_check
    CHECK (queue_name IN ('primary', 'retry', 'dead_letter'));

CREATE INDEX IF NOT EXISTS idx_merchant_webhook_deliveries_retry_queue
    ON merchant_webhook_deliveries (queue_name, next_retry_at)
    WHERE status IN ('pending', 'retrying');

CREATE INDEX IF NOT EXISTS idx_merchant_webhook_deliveries_dead_letter
    ON merchant_webhook_deliveries (dead_lettered_at DESC)
    WHERE status = 'dead_lettered';

CREATE TABLE IF NOT EXISTS merchant_webhook_dead_letters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webhook_delivery_id UUID NOT NULL UNIQUE REFERENCES merchant_webhook_deliveries(id) ON DELETE CASCADE,
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    webhook_url TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    last_error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    operator_alert_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (operator_alert_status IN ('pending', 'sent', 'acknowledged')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_merchant_webhook_dead_letters_alert
    ON merchant_webhook_dead_letters (operator_alert_status, created_at);

CREATE TABLE IF NOT EXISTS merchant_webhook_endpoint_circuits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    webhook_url TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'closed'
        CHECK (state IN ('closed', 'open', 'half_open')),
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    opened_until TIMESTAMPTZ,
    last_failure_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (merchant_id, webhook_url)
);

CREATE INDEX IF NOT EXISTS idx_merchant_webhook_endpoint_circuits_open
    ON merchant_webhook_endpoint_circuits (opened_until)
    WHERE state = 'open';

CREATE TRIGGER set_updated_at_merchant_webhook_dead_letters
    BEFORE UPDATE ON merchant_webhook_dead_letters
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_merchant_webhook_endpoint_circuits
    BEFORE UPDATE ON merchant_webhook_endpoint_circuits
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
