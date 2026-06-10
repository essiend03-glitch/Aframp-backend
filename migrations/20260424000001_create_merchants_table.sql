-- Merchants base table (stub used by loyalty rewards, webhooks, and other merchant modules)
-- The full merchant schema is extended by later migrations (merchant_crm, pos, etc.)
CREATE TABLE IF NOT EXISTS merchants (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL,
    email           TEXT UNIQUE NOT NULL,
    status          TEXT NOT NULL DEFAULT 'active',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_merchants_status ON merchants(status);
CREATE INDEX IF NOT EXISTS idx_merchants_email  ON merchants(email);
