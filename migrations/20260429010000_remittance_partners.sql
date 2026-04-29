-- migrate:up
-- Remittance Partner Integration & White-label Support (Issue #408)

CREATE TABLE remittance_partners (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,          -- machine-readable identifier used in API keys
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'pending')),
    api_key_hash TEXT NOT NULL UNIQUE,  -- SHA-256 of the issued API key
    webhook_url TEXT,
    webhook_secret TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE partner_branding (
    partner_id UUID PRIMARY KEY REFERENCES remittance_partners(id) ON DELETE CASCADE,
    logo_url TEXT,
    primary_color TEXT,
    secondary_color TEXT,
    email_template JSONB NOT NULL DEFAULT '{}'::jsonb,  -- {subject, body_html}
    language_overrides JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE partner_fee_structures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id UUID NOT NULL REFERENCES remittance_partners(id) ON DELETE CASCADE,
    corridor TEXT NOT NULL,             -- e.g. "NGN->KES"
    fee_type TEXT NOT NULL CHECK (fee_type IN ('percent', 'flat')),
    fee_value NUMERIC(18, 6) NOT NULL CHECK (fee_value >= 0),
    min_amount NUMERIC(36, 18),
    max_amount NUMERIC(36, 18),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (partner_id, corridor)
);

CREATE TABLE partner_limits (
    partner_id UUID PRIMARY KEY REFERENCES remittance_partners(id) ON DELETE CASCADE,
    daily_volume_limit NUMERIC(36, 18),
    per_tx_min NUMERIC(36, 18) NOT NULL DEFAULT 0,
    per_tx_max NUMERIC(36, 18),
    kyc_threshold NUMERIC(36, 18),      -- transfers above this require enhanced KYC
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE partner_liquidity_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id UUID NOT NULL REFERENCES remittance_partners(id) ON DELETE CASCADE,
    currency TEXT NOT NULL,
    stellar_address TEXT,
    balance NUMERIC(36, 18) NOT NULL DEFAULT 0,
    reserved NUMERIC(36, 18) NOT NULL DEFAULT 0,  -- funds locked in in-flight transfers
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (partner_id, currency)
);

CREATE TABLE partner_transfers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id UUID NOT NULL REFERENCES remittance_partners(id) ON DELETE RESTRICT,
    partner_ref TEXT NOT NULL,          -- partner's own reference
    from_currency TEXT NOT NULL,
    to_currency TEXT NOT NULL,
    from_amount NUMERIC(36, 18) NOT NULL CHECK (from_amount > 0),
    to_amount NUMERIC(36, 18) NOT NULL CHECK (to_amount > 0),
    fee_amount NUMERIC(36, 18) NOT NULL DEFAULT 0,
    fx_rate NUMERIC(24, 10) NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'cancelled')),
    stellar_tx_hash TEXT,
    error_message TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (partner_id, partner_ref)
);

CREATE TABLE partner_settlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id UUID NOT NULL REFERENCES remittance_partners(id) ON DELETE RESTRICT,
    settlement_date DATE NOT NULL,
    total_volume NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_fees NUMERIC(36, 18) NOT NULL DEFAULT 0,
    net_payable NUMERIC(36, 18) NOT NULL DEFAULT 0,  -- positive = Aframp owes partner
    tx_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sent', 'confirmed')),
    report_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (partner_id, settlement_date)
);

-- Indexes
CREATE INDEX idx_partner_transfers_partner ON partner_transfers(partner_id, created_at DESC);
CREATE INDEX idx_partner_transfers_status ON partner_transfers(status);
CREATE INDEX idx_partner_settlements_partner ON partner_settlements(partner_id, settlement_date DESC);

-- Triggers
CREATE TRIGGER set_updated_at_partners
    BEFORE UPDATE ON remittance_partners FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER set_updated_at_partner_fees
    BEFORE UPDATE ON partner_fee_structures FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER set_updated_at_partner_transfers
    BEFORE UPDATE ON partner_transfers FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER set_updated_at_partner_settlements
    BEFORE UPDATE ON partner_settlements FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- migrate:down
DROP TABLE IF EXISTS partner_settlements;
DROP TABLE IF EXISTS partner_transfers;
DROP TABLE IF EXISTS partner_liquidity_accounts;
DROP TABLE IF EXISTS partner_limits;
DROP TABLE IF EXISTS partner_fee_structures;
DROP TABLE IF EXISTS partner_branding;
DROP TABLE IF EXISTS remittance_partners;
