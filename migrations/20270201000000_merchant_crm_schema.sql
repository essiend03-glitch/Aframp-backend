-- =============================================================================
-- Issue #334: Merchant CRM & Customer Insights
-- =============================================================================

-- Customer profiles derived from wallet transaction clustering
CREATE TABLE merchant_customer_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    wallet_address VARCHAR(255) NOT NULL,
    display_name TEXT,
    -- Consent-based contact details (AES-256 encrypted at rest)
    encrypted_email TEXT,
    encrypted_phone TEXT,
    encrypted_name TEXT,
    consent_given BOOLEAN NOT NULL DEFAULT FALSE,
    consent_given_at TIMESTAMPTZ,
    consent_ip_address INET,
    -- Lifetime value metrics
    total_spent NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_transactions INTEGER NOT NULL DEFAULT 0,
    first_transaction_at TIMESTAMPTZ,
    last_transaction_at TIMESTAMPTZ,
    -- Retention
    is_repeat_customer BOOLEAN NOT NULL DEFAULT FALSE,
    -- Tags (e.g. "VIP", "Churned", "First-Time-Buyer")
    tags TEXT[] NOT NULL DEFAULT '{}',
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, wallet_address)
);

COMMENT ON TABLE merchant_customer_profiles IS 'CRM profiles clustering transactions by wallet per merchant.';

-- Customer segments for filtering
CREATE TABLE merchant_customer_segments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    -- Filter criteria stored as JSONB (e.g. min_spent, date_range, tags)
    filter_criteria JSONB NOT NULL DEFAULT '{}',
    customer_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, name)
);

COMMENT ON TABLE merchant_customer_segments IS 'Named customer segments with filter criteria for merchant CRM.';

-- Purchasing pattern analytics snapshots
CREATE TABLE merchant_customer_analytics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    wallet_address VARCHAR(255) NOT NULL,
    -- Frequency metrics
    avg_purchase_frequency_days NUMERIC(10, 4),
    days_since_last_purchase INTEGER,
    -- Spend metrics
    avg_transaction_value NUMERIC(36, 18),
    max_transaction_value NUMERIC(36, 18),
    min_transaction_value NUMERIC(36, 18),
    -- Retention
    retention_score NUMERIC(5, 4) CHECK (retention_score BETWEEN 0 AND 1),
    snapshot_date DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, wallet_address, snapshot_date)
);

COMMENT ON TABLE merchant_customer_analytics IS 'Daily analytics snapshots per customer per merchant.';

-- Indexes
CREATE INDEX idx_mcp_merchant_id ON merchant_customer_profiles(merchant_id);
CREATE INDEX idx_mcp_wallet ON merchant_customer_profiles(wallet_address);
CREATE INDEX idx_mcp_last_tx ON merchant_customer_profiles(last_transaction_at);
CREATE INDEX idx_mcp_tags ON merchant_customer_profiles USING GIN(tags);
CREATE INDEX idx_mca_merchant_wallet ON merchant_customer_analytics(merchant_id, wallet_address);
CREATE INDEX idx_mca_snapshot_date ON merchant_customer_analytics(snapshot_date);

-- Triggers
CREATE TRIGGER set_updated_at_merchant_customer_profiles
    BEFORE UPDATE ON merchant_customer_profiles
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_merchant_customer_segments
    BEFORE UPDATE ON merchant_customer_segments
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
