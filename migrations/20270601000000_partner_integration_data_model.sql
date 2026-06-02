-- Migration: Partner Integration Framework — Data Model (Issue #466)
-- Adds partners, partner_profiles, and partner_api_credentials tables for
-- multi-tenant partner onboarding with compliance tiers and asymmetric key support.

-- ── Partners ──────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS partners (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Business classification
    legal_name          TEXT NOT NULL,
    trading_name        TEXT,
    organisation_type   TEXT NOT NULL
                            CHECK (organisation_type IN (
                                'commercial_bank', 'mobile_money_operator',
                                'fintech', 'microfinance', 'payment_aggregator', 'other'
                            )),
    registration_number TEXT NOT NULL,
    jurisdiction        TEXT NOT NULL,  -- ISO 3166-1 alpha-2 (e.g. 'NG', 'KE')
    -- Onboarding lifecycle
    onboarding_state    TEXT NOT NULL DEFAULT 'sandbox'
                            CHECK (onboarding_state IN ('sandbox', 'testing', 'verified', 'production')),
    -- Compliance tier drives AML/KYB scrutiny level
    compliance_tier     TEXT NOT NULL DEFAULT 'standard'
                            CHECK (compliance_tier IN ('standard', 'enhanced', 'premium')),
    -- Tenant isolation
    tenant_id           UUID NOT NULL DEFAULT gen_random_uuid(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE partners IS 'Core partner registry for the Partner Integration Framework (Issue #466)';
COMMENT ON COLUMN partners.onboarding_state IS 'sandbox → testing → verified → production lifecycle';
COMMENT ON COLUMN partners.compliance_tier IS 'standard | enhanced | premium — governs AML/KYB scrutiny';
COMMENT ON COLUMN partners.tenant_id IS 'Immutable tenant isolation key for multi-tenant data segregation';

CREATE UNIQUE INDEX IF NOT EXISTS uidx_partners_registration
    ON partners(registration_number, jurisdiction);
CREATE INDEX IF NOT EXISTS idx_partners_onboarding_state
    ON partners(onboarding_state);
CREATE INDEX IF NOT EXISTS idx_partners_tenant_id
    ON partners(tenant_id);

-- ── Partner profiles ──────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS partner_profiles (
    partner_id              UUID PRIMARY KEY REFERENCES partners(id) ON DELETE CASCADE,
    -- Contact matrix
    primary_contact_name    TEXT NOT NULL,
    primary_contact_email   TEXT NOT NULL,
    primary_contact_phone   TEXT,
    technical_contact_email TEXT,
    compliance_contact_email TEXT,
    -- Business details
    website_url             TEXT,
    support_url             TEXT,
    logo_url                TEXT,
    -- Regulatory
    regulatory_licence_ref  TEXT,
    regulated_by            TEXT,  -- e.g. 'CBN', 'FCA', 'RBZ'
    -- Metadata
    notes                   TEXT,
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE partner_profiles IS 'Extended business and contact details for each partner';
COMMENT ON COLUMN partner_profiles.compliance_contact_email IS 'Dedicated compliance officer contact for AML/KYB escalations';

-- ── Partner API credentials ───────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS partner_api_credentials (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id              UUID NOT NULL REFERENCES partners(id) ON DELETE CASCADE,
    -- Salted API key hash (Argon2id or bcrypt — never plaintext)
    api_key_hash            TEXT NOT NULL,
    api_key_salt            TEXT NOT NULL,
    api_key_prefix          TEXT NOT NULL UNIQUE,  -- first 8 chars for fast lookup
    -- Asymmetric public signing key (Ed25519 or RSA-2048 PEM)
    public_signing_key      TEXT,
    signing_algorithm       TEXT DEFAULT 'Ed25519'
                                CHECK (signing_algorithm IN ('Ed25519', 'RSA-2048', 'ES256')),
    -- Network controls
    ip_whitelist            INET[] NOT NULL DEFAULT '{}',
    -- Webhook delivery
    webhook_url             TEXT,
    webhook_secret_hash     TEXT,  -- HMAC-SHA256 secret hash for payload signing
    -- Lifecycle
    environment             TEXT NOT NULL DEFAULT 'sandbox'
                                CHECK (environment IN ('sandbox', 'production')),
    expires_at              TIMESTAMPTZ,
    revoked_at              TIMESTAMPTZ,
    last_used_at            TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE partner_api_credentials IS 'Secure API credentials with salted hashes, asymmetric keys, IP whitelists, and webhook config';
COMMENT ON COLUMN partner_api_credentials.api_key_hash IS 'Argon2id hash of the full API key — raw key returned once at issuance';
COMMENT ON COLUMN partner_api_credentials.api_key_salt IS 'Per-credential random salt used in the Argon2id hash';
COMMENT ON COLUMN partner_api_credentials.public_signing_key IS 'PEM-encoded public key for verifying partner-signed request payloads';
COMMENT ON COLUMN partner_api_credentials.ip_whitelist IS 'CIDR/IP list; empty array means no IP restriction (sandbox only)';
COMMENT ON COLUMN partner_api_credentials.webhook_secret_hash IS 'SHA-256 of the HMAC secret used to sign outbound webhook payloads';

CREATE INDEX IF NOT EXISTS idx_partner_api_creds_partner_id
    ON partner_api_credentials(partner_id);
CREATE INDEX IF NOT EXISTS idx_partner_api_creds_prefix
    ON partner_api_credentials(api_key_prefix);
CREATE INDEX IF NOT EXISTS idx_partner_api_creds_active
    ON partner_api_credentials(partner_id)
    WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now());

-- ── updated_at triggers ───────────────────────────────────────────────────────

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'partners_updated_at') THEN
        CREATE TRIGGER partners_updated_at
            BEFORE UPDATE ON partners
            FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'partner_profiles_updated_at') THEN
        CREATE TRIGGER partner_profiles_updated_at
            BEFORE UPDATE ON partner_profiles
            FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
END;
$$;
