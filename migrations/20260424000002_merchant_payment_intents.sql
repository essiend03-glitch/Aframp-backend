-- merchant_payment_intents: QR/POS payment intents for merchant gateway
CREATE TABLE IF NOT EXISTS merchant_payment_intents (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id             UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    merchant_reference      TEXT NOT NULL,
    amount_cngn             NUMERIC(20, 8) NOT NULL,
    currency                TEXT NOT NULL DEFAULT 'NGN',

    -- Customer details
    customer_email          TEXT,
    customer_phone          TEXT,
    customer_address        TEXT,

    -- Blockchain details
    destination_address     TEXT NOT NULL,
    memo                    TEXT NOT NULL UNIQUE,
    stellar_tx_hash         TEXT,
    actual_amount_received  NUMERIC(20, 8),

    -- Status
    status                  TEXT NOT NULL DEFAULT 'pending'
                                CHECK (status IN ('pending', 'paid', 'expired', 'cancelled', 'refunded')),

    -- Timing
    expires_at              TIMESTAMPTZ NOT NULL,
    paid_at                 TIMESTAMPTZ,
    confirmed_at            TIMESTAMPTZ,

    -- Metadata
    metadata                JSONB NOT NULL DEFAULT '{}',
    callback_url            TEXT,

    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mpi_merchant_id        ON merchant_payment_intents(merchant_id);
CREATE INDEX IF NOT EXISTS idx_mpi_status             ON merchant_payment_intents(status);
CREATE INDEX IF NOT EXISTS idx_mpi_merchant_reference ON merchant_payment_intents(merchant_id, merchant_reference);
CREATE INDEX IF NOT EXISTS idx_mpi_expires_at         ON merchant_payment_intents(expires_at);
CREATE INDEX IF NOT EXISTS idx_mpi_created_at         ON merchant_payment_intents(created_at DESC);
