-- migrate:up
-- Wallet analytics & usage pattern tracking (Issue #369)

-- Wallet usage snapshots (daily / weekly / monthly)
CREATE TABLE wallet_usage_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address VARCHAR(255) NOT NULL REFERENCES wallets(wallet_address) ON UPDATE CASCADE ON DELETE CASCADE,
    period TEXT NOT NULL CHECK (period IN ('daily', 'weekly', 'monthly')),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ NOT NULL,
    total_tx_count INTEGER NOT NULL DEFAULT 0,
    total_cngn_sent NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_cngn_received NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_fiat_onramped NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_fiat_offramped NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_fees_paid NUMERIC(36, 18) NOT NULL DEFAULT 0,
    unique_counterparties INTEGER NOT NULL DEFAULT 0,
    most_used_tx_type TEXT,
    most_used_provider TEXT,
    active_days INTEGER NOT NULL DEFAULT 0,
    snapshot_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_address, period, period_start)
);

-- Spending category breakdown per wallet per period
CREATE TABLE wallet_spending_categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address VARCHAR(255) NOT NULL REFERENCES wallets(wallet_address) ON UPDATE CASCADE ON DELETE CASCADE,
    period TEXT NOT NULL CHECK (period IN ('daily', 'weekly', 'monthly')),
    period_start TIMESTAMPTZ NOT NULL,
    category TEXT NOT NULL CHECK (category IN ('bill_payments', 'transfers', 'onramp', 'offramp')),
    tx_count INTEGER NOT NULL DEFAULT 0,
    total_amount NUMERIC(36, 18) NOT NULL DEFAULT 0,
    percentage NUMERIC(5, 2) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_address, period, period_start, category)
);

-- Counterparty frequency tracking
CREATE TABLE wallet_counterparty_frequency (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address VARCHAR(255) NOT NULL REFERENCES wallets(wallet_address) ON UPDATE CASCADE ON DELETE CASCADE,
    counterparty_id TEXT NOT NULL,
    counterparty_type TEXT NOT NULL CHECK (counterparty_type IN ('wallet', 'provider', 'bill_provider')),
    tx_count INTEGER NOT NULL DEFAULT 1,
    total_amount_sent NUMERIC(36, 18) NOT NULL DEFAULT 0,
    first_tx_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_tx_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_address, counterparty_id)
);

-- Wallet behaviour profiles
CREATE TABLE wallet_behaviour_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address VARCHAR(255) NOT NULL UNIQUE REFERENCES wallets(wallet_address) ON UPDATE CASCADE ON DELETE CASCADE,
    avg_tx_size NUMERIC(36, 18) NOT NULL DEFAULT 0,
    tx_frequency_per_week NUMERIC(10, 4) NOT NULL DEFAULT 0,
    preferred_hour_utc SMALLINT,  -- 0-23
    preferred_provider TEXT,
    preferred_currency_pair TEXT,
    risk_score NUMERIC(5, 2) NOT NULL DEFAULT 0 CHECK (risk_score >= 0 AND risk_score <= 100),
    profile_updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Personal financial insights
CREATE TABLE wallet_insights (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address VARCHAR(255) NOT NULL REFERENCES wallets(wallet_address) ON UPDATE CASCADE ON DELETE CASCADE,
    period TEXT NOT NULL CHECK (period IN ('weekly', 'monthly')),
    period_start TIMESTAMPTZ NOT NULL,
    top_category TEXT,
    top_category_amount NUMERIC(36, 18),
    prev_period_delta_pct NUMERIC(8, 2),
    largest_tx_amount NUMERIC(36, 18),
    largest_tx_id UUID,
    most_frequent_counterparty TEXT,
    estimated_monthly_fees NUMERIC(36, 18),
    cngn_balance_trend TEXT CHECK (cngn_balance_trend IN ('increasing', 'decreasing', 'stable')),
    generated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_address, period, period_start)
);

-- User insight delivery preferences
CREATE TABLE wallet_insight_preferences (
    wallet_address VARCHAR(255) PRIMARY KEY REFERENCES wallets(wallet_address) ON UPDATE CASCADE ON DELETE CASCADE,
    weekly_insights BOOLEAN NOT NULL DEFAULT TRUE,
    monthly_insights BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Pre-computed admin daily aggregates
CREATE TABLE admin_daily_aggregates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agg_date DATE NOT NULL UNIQUE,
    total_wallets BIGINT NOT NULL DEFAULT 0,
    active_wallets BIGINT NOT NULL DEFAULT 0,
    new_wallets BIGINT NOT NULL DEFAULT 0,
    total_cngn_transferred NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_fiat_onramped NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_fiat_offramped NUMERIC(36, 18) NOT NULL DEFAULT 0,
    avg_tx_size NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_tx_count BIGINT NOT NULL DEFAULT 0,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Anomaly flags for compliance routing
CREATE TABLE wallet_anomaly_flags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address VARCHAR(255) NOT NULL REFERENCES wallets(wallet_address) ON UPDATE CASCADE ON DELETE CASCADE,
    anomaly_type TEXT NOT NULL CHECK (anomaly_type IN ('volume_spike', 'size_shift', 'new_counterparty_rate', 'time_pattern_shift')),
    deviation_magnitude NUMERIC(10, 4) NOT NULL,
    flagged_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    routed_to_compliance BOOLEAN NOT NULL DEFAULT FALSE,
    compliance_case_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes
CREATE INDEX idx_snapshots_wallet_period ON wallet_usage_snapshots(wallet_address, period, period_start DESC);
CREATE INDEX idx_snapshots_period_start ON wallet_usage_snapshots(period, period_start);
CREATE INDEX idx_spending_wallet_period ON wallet_spending_categories(wallet_address, period, period_start DESC);
CREATE INDEX idx_counterparty_wallet ON wallet_counterparty_frequency(wallet_address, tx_count DESC);
CREATE INDEX idx_insights_wallet ON wallet_insights(wallet_address, period, period_start DESC);
CREATE INDEX idx_anomaly_wallet ON wallet_anomaly_flags(wallet_address, flagged_at DESC);
CREATE INDEX idx_anomaly_unresolved ON wallet_anomaly_flags(resolved_at) WHERE resolved_at IS NULL;
CREATE INDEX idx_admin_agg_date ON admin_daily_aggregates(agg_date DESC);

-- Triggers
CREATE TRIGGER set_updated_at_snapshots
    BEFORE UPDATE ON wallet_usage_snapshots
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_spending
    BEFORE UPDATE ON wallet_spending_categories
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_counterparty
    BEFORE UPDATE ON wallet_counterparty_frequency
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_profiles
    BEFORE UPDATE ON wallet_behaviour_profiles
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_anomaly
    BEFORE UPDATE ON wallet_anomaly_flags
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- migrate:down
DROP TABLE IF EXISTS wallet_anomaly_flags;
DROP TABLE IF EXISTS admin_daily_aggregates;
DROP TABLE IF EXISTS wallet_insight_preferences;
DROP TABLE IF EXISTS wallet_insights;
DROP TABLE IF EXISTS wallet_behaviour_profiles;
DROP TABLE IF EXISTS wallet_counterparty_frequency;
DROP TABLE IF EXISTS wallet_spending_categories;
DROP TABLE IF EXISTS wallet_usage_snapshots;
