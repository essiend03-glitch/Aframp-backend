-- ============================================================================
-- Merchant Loyalty & Cashback Rewards Engine
-- Issue #332
-- ============================================================================
-- Enables merchants to configure cashback campaigns such as:
-- IF Transaction > X THEN Send Y% Cashback.
--
-- Rewards reserve campaign budget atomically before Stellar payout submission.
-- High-risk wallets and velocity patterns move rewards to compliance hold.
-- ============================================================================

CREATE TABLE IF NOT EXISTS merchant_loyalty_campaigns (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'active', 'paused', 'deactivated', 'exhausted')),
    trigger_min_amount_cngn DECIMAL(18, 8) NOT NULL CHECK (trigger_min_amount_cngn > 0),
    cashback_percent DECIMAL(9, 4) NOT NULL CHECK (cashback_percent > 0 AND cashback_percent <= 100),
    budget_cap_cngn DECIMAL(18, 8) NOT NULL CHECK (budget_cap_cngn > 0),
    budget_spent_cngn DECIMAL(18, 8) NOT NULL DEFAULT 0 CHECK (budget_spent_cngn >= 0),
    per_customer_daily_cap_cngn DECIMAL(18, 8) CHECK (per_customer_daily_cap_cngn IS NULL OR per_customer_daily_cap_cngn > 0),
    segment_tags TEXT[] NOT NULL DEFAULT '{}',
    vip_cashback_multiplier DECIMAL(9, 4) NOT NULL DEFAULT 1 CHECK (vip_cashback_multiplier >= 1 AND vip_cashback_multiplier <= 5),
    stellar_source_account VARCHAR(69),
    atomic_stellar_enabled BOOLEAN NOT NULL DEFAULT true,
    starts_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ends_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}',
    activated_at TIMESTAMPTZ,
    deactivated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT loyalty_campaign_budget_spend_lte_cap CHECK (budget_spent_cngn <= budget_cap_cngn),
    CONSTRAINT loyalty_campaign_valid_window CHECK (ends_at IS NULL OR ends_at > starts_at)
);

CREATE INDEX IF NOT EXISTS idx_loyalty_campaigns_merchant_status
    ON merchant_loyalty_campaigns (merchant_id, status, starts_at, ends_at);
CREATE INDEX IF NOT EXISTS idx_loyalty_campaigns_segments
    ON merchant_loyalty_campaigns USING GIN (segment_tags);

CREATE TABLE IF NOT EXISTS merchant_loyalty_rewards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL REFERENCES merchant_loyalty_campaigns(id) ON DELETE CASCADE,
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    payment_intent_id UUID NOT NULL REFERENCES merchant_payment_intents(id) ON DELETE CASCADE,
    customer_address VARCHAR(69) NOT NULL,
    transaction_amount_cngn DECIMAL(18, 8) NOT NULL CHECK (transaction_amount_cngn > 0),
    reward_amount_cngn DECIMAL(18, 8) NOT NULL CHECK (reward_amount_cngn > 0),
    cashback_percent DECIMAL(9, 4) NOT NULL CHECK (cashback_percent > 0),
    customer_tier TEXT NOT NULL DEFAULT 'standard' CHECK (customer_tier IN ('standard', 'vip')),
    risk_status TEXT NOT NULL DEFAULT 'clear' CHECK (risk_status IN ('clear', 'flagged', 'high_risk')),
    risk_flags TEXT[] NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'submitted', 'paid', 'held', 'failed')),
    stellar_tx_hash VARCHAR(128),
    stellar_source_account VARCHAR(69),
    idempotency_key TEXT NOT NULL,
    atomicity_mode TEXT NOT NULL DEFAULT 'post_receipt_queue'
        CHECK (atomicity_mode IN ('stellar_payment_channel', 'post_receipt_queue')),
    notification_status TEXT NOT NULL DEFAULT 'queued'
        CHECK (notification_status IN ('queued', 'sent', 'failed')),
    failure_code TEXT,
    paid_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (campaign_id, payment_intent_id),
    UNIQUE (idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_loyalty_rewards_campaign
    ON merchant_loyalty_rewards (campaign_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_loyalty_rewards_merchant_status
    ON merchant_loyalty_rewards (merchant_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_loyalty_rewards_customer_velocity
    ON merchant_loyalty_rewards (merchant_id, customer_address, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_loyalty_rewards_risk
    ON merchant_loyalty_rewards (risk_status, created_at DESC)
    WHERE risk_status <> 'clear';

CREATE TABLE IF NOT EXISTS merchant_loyalty_risk_wallets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address VARCHAR(69) NOT NULL UNIQUE,
    risk_label TEXT NOT NULL,
    internal_reason TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_loyalty_risk_wallets_active
    ON merchant_loyalty_risk_wallets (wallet_address)
    WHERE is_active = true;

CREATE TABLE IF NOT EXISTS merchant_loyalty_reward_notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reward_id UUID NOT NULL REFERENCES merchant_loyalty_rewards(id) ON DELETE CASCADE,
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    customer_address VARCHAR(69) NOT NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sent', 'failed')),
    sent_at TIMESTAMPTZ,
    failure_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (reward_id, event_type)
);

CREATE INDEX IF NOT EXISTS idx_loyalty_notifications_status
    ON merchant_loyalty_reward_notifications (status, created_at);
CREATE INDEX IF NOT EXISTS idx_loyalty_notifications_customer
    ON merchant_loyalty_reward_notifications (merchant_id, customer_address, created_at DESC);

CREATE OR REPLACE VIEW merchant_loyalty_marketing_spend_daily AS
SELECT
    merchant_id,
    campaign_id,
    DATE_TRUNC('day', created_at)::date AS spend_date,
    COUNT(*) AS reward_count,
    SUM(reward_amount_cngn) AS total_reward_amount_cngn,
    SUM(reward_amount_cngn) FILTER (WHERE status = 'paid') AS paid_reward_amount_cngn,
    COUNT(*) FILTER (WHERE status = 'held') AS held_reward_count,
    COUNT(*) FILTER (WHERE risk_status <> 'clear') AS risk_flagged_count
FROM merchant_loyalty_rewards
GROUP BY merchant_id, campaign_id, DATE_TRUNC('day', created_at)::date;

CREATE TRIGGER set_updated_at_merchant_loyalty_campaigns
    BEFORE UPDATE ON merchant_loyalty_campaigns
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_merchant_loyalty_rewards
    BEFORE UPDATE ON merchant_loyalty_rewards
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_merchant_loyalty_risk_wallets
    BEFORE UPDATE ON merchant_loyalty_risk_wallets
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_merchant_loyalty_reward_notifications
    BEFORE UPDATE ON merchant_loyalty_reward_notifications
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
