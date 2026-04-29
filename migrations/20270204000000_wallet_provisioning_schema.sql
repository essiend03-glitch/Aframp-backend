-- =============================================================================
-- Issue #322: Wallet Creation & Stellar Account Provisioning
-- =============================================================================

-- Provisioning state machine per wallet
CREATE TABLE wallet_provisioning (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id UUID NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    -- State machine
    state TEXT NOT NULL DEFAULT 'keypair_generated'
        CHECK (state IN (
            'keypair_generated',
            'registered',
            'pending_funding',
            'funded',
            'trustline_pending',
            'trustline_active',
            'ready',
            'stalled',
            'failed'
        )),
    -- Sponsorship
    is_sponsored BOOLEAN NOT NULL DEFAULT FALSE,
    sponsorship_tx_hash TEXT,
    sponsorship_xlm_amount NUMERIC(18, 7),
    -- Funding
    funding_method TEXT CHECK (funding_method IN ('self_funded', 'sponsored', 'exchange')),
    funding_detected_at TIMESTAMPTZ,
    funding_tx_hash TEXT,
    -- Trustline
    trustline_envelope TEXT,                       -- Unsigned XDR envelope for client signing
    trustline_submitted_at TIMESTAMPTZ,
    trustline_tx_hash TEXT,
    trustline_authorized_at TIMESTAMPTZ,
    -- Readiness
    became_ready_at TIMESTAMPTZ,
    -- Failure tracking
    last_failure_reason TEXT,
    last_failure_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0,
    -- Timeout tracking
    step_started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    step_timeout_at TIMESTAMPTZ,
    -- Metadata
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_id)
);

COMMENT ON TABLE wallet_provisioning IS 'Resumable provisioning state machine for Stellar wallet setup.';

-- Provisioning state transition history
CREATE TABLE wallet_provisioning_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id UUID NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    from_state TEXT,
    to_state TEXT NOT NULL,
    transition_reason TEXT,
    triggered_by TEXT,                             -- 'user', 'system', 'worker'
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE wallet_provisioning_history IS 'Immutable audit trail of all provisioning state transitions.';

-- Platform funding account management
CREATE TABLE platform_funding_account (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stellar_address VARCHAR(255) NOT NULL UNIQUE,
    current_xlm_balance NUMERIC(18, 7) NOT NULL DEFAULT 0,
    total_accounts_sponsored INTEGER NOT NULL DEFAULT 0,
    total_xlm_spent NUMERIC(18, 7) NOT NULL DEFAULT 0,
    -- Alert threshold
    min_balance_alert_threshold NUMERIC(18, 7) NOT NULL DEFAULT 100,
    -- Sponsorship eligibility config
    eligibility_criteria JSONB NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    last_balance_check_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE platform_funding_account IS 'Platform XLM funding account for sponsored wallet creation.';

-- Funding account replenishment requests
CREATE TABLE funding_account_replenishment_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    funding_account_id UUID NOT NULL REFERENCES platform_funding_account(id) ON DELETE CASCADE,
    requested_xlm_amount NUMERIC(18, 7) NOT NULL,
    requested_by UUID REFERENCES users(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'completed', 'rejected')),
    notes TEXT,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE funding_account_replenishment_requests IS 'Replenishment requests for the platform XLM funding account.';

-- Wallet readiness criteria cache
CREATE TABLE wallet_readiness_checks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id UUID NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    stellar_account_exists BOOLEAN NOT NULL DEFAULT FALSE,
    min_xlm_balance_met BOOLEAN NOT NULL DEFAULT FALSE,
    trustline_active BOOLEAN NOT NULL DEFAULT FALSE,
    trustline_authorized BOOLEAN NOT NULL DEFAULT FALSE,
    wallet_registered BOOLEAN NOT NULL DEFAULT FALSE,
    all_criteria_met BOOLEAN NOT NULL DEFAULT FALSE,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (wallet_id)
);

COMMENT ON TABLE wallet_readiness_checks IS 'Cached readiness criteria evaluation per wallet.';

-- Indexes
CREATE INDEX idx_wp_wallet_id ON wallet_provisioning(wallet_id);
CREATE INDEX idx_wp_state ON wallet_provisioning(state);
CREATE INDEX idx_wp_step_timeout ON wallet_provisioning(step_timeout_at) WHERE state NOT IN ('ready', 'failed');
CREATE INDEX idx_wph_wallet_id ON wallet_provisioning_history(wallet_id);
CREATE INDEX idx_wph_created_at ON wallet_provisioning_history(created_at);
CREATE INDEX idx_wrc_wallet_id ON wallet_readiness_checks(wallet_id);

-- Triggers
CREATE TRIGGER set_updated_at_wallet_provisioning
    BEFORE UPDATE ON wallet_provisioning
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_platform_funding_account
    BEFORE UPDATE ON platform_funding_account
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_funding_account_replenishment_requests
    BEFORE UPDATE ON funding_account_replenishment_requests
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
