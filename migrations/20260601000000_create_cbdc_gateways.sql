-- migrate:up
-- CBDC Network Gateway Registry (Issue #499)
-- Tracks permissioned central bank DLT node endpoints, connection profiles,
-- mTLS certificate footprints, and operational status.

CREATE TABLE IF NOT EXISTS cbdc_gateways (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                        TEXT NOT NULL,
    description                 TEXT,
    dlt_system                  TEXT NOT NULL CHECK (dlt_system IN ('Hyperledger Besu', 'Corda', 'Quorum', 'Hyperledger Fabric')),
    network_type                TEXT NOT NULL DEFAULT 'sandbox' CHECK (network_type IN ('sandbox', 'staging', 'production')),
    rpc_endpoint                TEXT NOT NULL,
    ws_endpoint                 TEXT,
    chain_id                    BIGINT,
    mtls_certificate_footprint  TEXT,
    mtls_ca_cert_pem            TEXT,
    mtls_client_cert_pem        TEXT,
    node_identity               TEXT,
    connection_timeout_ms       INTEGER NOT NULL DEFAULT 5000,
    max_retries                 INTEGER NOT NULL DEFAULT 3,
    retry_backoff_ms            INTEGER NOT NULL DEFAULT 1000,
    rate_limit_rps              INTEGER NOT NULL DEFAULT 10,
    is_active                   BOOLEAN NOT NULL DEFAULT TRUE,
    last_health_check_at        TIMESTAMPTZ,
    last_healthy_at             TIMESTAMPTZ,
    health_status               TEXT DEFAULT 'unknown' CHECK (health_status IN ('healthy', 'degraded', 'unreachable', 'unknown')),
    metadata                    JSONB DEFAULT '{}'::jsonb,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cbdc_gateways_dlt_system ON cbdc_gateways(dlt_system);
CREATE INDEX IF NOT EXISTS idx_cbdc_gateways_status ON cbdc_gateways(health_status);
CREATE INDEX IF NOT EXISTS idx_cbdc_gateways_active ON cbdc_gateways(is_active) WHERE is_active = TRUE;

COMMENT ON TABLE cbdc_gateways IS 'CBDC Network Gateway Registry — permissioned central bank DLT node endpoints';
COMMENT ON COLUMN cbdc_gateways.dlt_system IS 'Enterprise DLT platform type: Hyperledger Besu, Corda, Quorum, or Hyperledger Fabric';
COMMENT ON COLUMN cbdc_gateways.mtls_certificate_footprint IS 'SHA-256 fingerprint of the mTLS client certificate for node authentication';
COMMENT ON COLUMN cbdc_gateways.health_status IS 'Current gateway health status as determined by the background health checker';
COMMENT ON COLUMN cbdc_gateways.metadata IS 'Flexible JSONB metadata for network-specific configuration and vendor extensions';

CREATE TRIGGER update_cbdc_gateways_updated_at
    BEFORE UPDATE ON cbdc_gateways
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- migrate:down
DROP TRIGGER IF EXISTS update_cbdc_gateways_updated_at ON cbdc_gateways;
DROP TABLE IF EXISTS cbdc_gateways;
