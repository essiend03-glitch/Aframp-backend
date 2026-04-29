-- Migration: Partner Integration Framework
-- Unified "Partner Hub" — banking partners, fintechs, liquidity providers.
-- Provides per-partner rate limiting, credential management (OAuth2/mTLS/API key),
-- API version deprecation tracking, and audit-ready correlation IDs.

-- ── Partner registry ──────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS integration_partners (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                  TEXT NOT NULL,
    organisation          TEXT NOT NULL UNIQUE,
    partner_type          TEXT NOT NULL CHECK (partner_type IN ('bank', 'fintech', 'liquidity_provider')),
    status                TEXT NOT NULL DEFAULT 'sandbox'
                              CHECK (status IN ('sandbox', 'active', 'suspended', 'deprecated')),
    contact_email         TEXT NOT NULL,
    ip_whitelist          TEXT[] NOT NULL DEFAULT '{}',
    rate_limit_per_minute INTEGER NOT NULL DEFAULT 500 CHECK (rate_limit_per_minute > 0),
    api_version           TEXT NOT NULL DEFAULT 'v1',
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE integration_partners IS 'External partner registry for the Partner Integration Framework';
COMMENT ON COLUMN integration_partners.partner_type IS 'bank | fintech | liquidity_provider';
COMMENT ON COLUMN integration_partners.status IS 'sandbox → active → suspended | deprecated';
COMMENT ON COLUMN integration_partners.ip_whitelist IS 'Allowed source IPs for Zero-Trust enforcement';
COMMENT ON COLUMN integration_partners.rate_limit_per_minute IS 'Per-partner request cap per 60-second window';

CREATE INDEX IF NOT EXISTS idx_integration_partners_organisation
    ON integration_partners(organisation);
CREATE INDEX IF NOT EXISTS idx_integration_partners_status
    ON integration_partners(status);

-- ── Partner credentials ───────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS partner_credentials (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id              UUID NOT NULL REFERENCES integration_partners(id) ON DELETE CASCADE,
    credential_type         TEXT NOT NULL CHECK (credential_type IN ('oauth2_client', 'mtls_cert', 'api_key')),
    -- OAuth2 fields
    client_id               TEXT UNIQUE,
    client_secret_hash      TEXT,
    -- mTLS fields
    certificate_fingerprint TEXT UNIQUE,
    -- API key fields
    api_key_hash            TEXT,
    api_key_prefix          TEXT UNIQUE,
    -- Common
    scopes                  TEXT[] NOT NULL DEFAULT '{}',
    environment             TEXT NOT NULL DEFAULT 'sandbox'
                                CHECK (environment IN ('sandbox', 'production')),
    expires_at              TIMESTAMPTZ,
    revoked_at              TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE partner_credentials IS 'OAuth2 clients, mTLS certificates, and API keys for partner auth';
COMMENT ON COLUMN partner_credentials.client_secret_hash IS 'SHA-256 of the raw client secret — never stored in plaintext';
COMMENT ON COLUMN partner_credentials.api_key_hash IS 'SHA-256 of the full API key — prefix stored separately for lookup';
COMMENT ON COLUMN partner_credentials.certificate_fingerprint IS 'SHA-256 of the PEM certificate — forwarded by TLS terminator as X-Client-Cert-Fingerprint';

CREATE INDEX IF NOT EXISTS idx_partner_credentials_partner_id
    ON partner_credentials(partner_id);
CREATE INDEX IF NOT EXISTS idx_partner_credentials_client_id
    ON partner_credentials(client_id) WHERE client_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_partner_credentials_api_key_prefix
    ON partner_credentials(api_key_prefix) WHERE api_key_prefix IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_partner_credentials_cert_fingerprint
    ON partner_credentials(certificate_fingerprint) WHERE certificate_fingerprint IS NOT NULL;

-- ── Per-partner rate counters (sliding minute window) ─────────────────────────

CREATE TABLE IF NOT EXISTS partner_rate_counters (
    partner_id     UUID NOT NULL REFERENCES integration_partners(id) ON DELETE CASCADE,
    window_start   TIMESTAMPTZ NOT NULL,
    request_count  BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (partner_id, window_start)
);

COMMENT ON TABLE partner_rate_counters IS 'Per-partner per-minute request counters for rate limiting';

CREATE INDEX IF NOT EXISTS idx_partner_rate_counters_window
    ON partner_rate_counters(window_start);

-- Purge stale windows older than 5 minutes (run via maintenance worker)
-- DELETE FROM partner_rate_counters WHERE window_start < now() - INTERVAL '5 minutes';

-- ── API version deprecation registry ─────────────────────────────────────────

CREATE TABLE IF NOT EXISTS api_version_deprecations (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    api_version          TEXT NOT NULL UNIQUE,
    deprecated_at        TIMESTAMPTZ NOT NULL,
    sunset_at            TIMESTAMPTZ NOT NULL,
    migration_guide_url  TEXT,
    notified_at          TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE api_version_deprecations IS 'Graceful deprecation schedule — partners are notified via the hub and response headers';
COMMENT ON COLUMN api_version_deprecations.sunset_at IS 'Date after which the version is removed; partners must migrate before this';
COMMENT ON COLUMN api_version_deprecations.notified_at IS 'Timestamp of last bulk notification dispatch to affected partners';

CREATE INDEX IF NOT EXISTS idx_api_version_deprecations_sunset
    ON api_version_deprecations(sunset_at);

-- Seed: mark v0 as deprecated (example)
INSERT INTO api_version_deprecations (api_version, deprecated_at, sunset_at, migration_guide_url)
VALUES (
    'v0',
    now(),
    now() + INTERVAL '90 days',
    'https://developers.aframp.io/migration/v0-to-v1'
)
ON CONFLICT (api_version) DO NOTHING;

-- ── updated_at trigger ────────────────────────────────────────────────────────

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'integration_partners_updated_at'
    ) THEN
        CREATE TRIGGER integration_partners_updated_at
            BEFORE UPDATE ON integration_partners
            FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
END;
$$;
