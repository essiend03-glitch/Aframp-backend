-- migrate:up
-- Cryptographic Signatory Vault (Issue #499)
-- Tracks multi-sig approval states required for sovereign-tier token exchanges
-- with strict data residency and partitioning rules.

CREATE TABLE IF NOT EXISTS cryptographic_signatory_vault (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swap_record_id      UUID NOT NULL REFERENCES cbdc_swap_records(id) ON DELETE CASCADE,
    signatory_type      TEXT NOT NULL CHECK (signatory_type IN (
        'central_bank_official', 'treasury_controller', 'compliance_officer',
        'platform_admin', 'auditor', 'hsm_module'
    )),
    signatory_identity  TEXT NOT NULL,
    signing_key_id      TEXT,
    signing_algorithm   TEXT NOT NULL DEFAULT 'ECDSA-P256' CHECK (signing_algorithm IN (
        'ECDSA-P256', 'ECDSA-P384', 'ED25519', 'RSA-2048', 'RSA-4096',
        'PKCS11-HSM'
    )),
    signature_value     TEXT,
    signature_payload   TEXT,
    signature_hash      TEXT,
    approval_action     TEXT NOT NULL CHECK (approval_action IN ('approve', 'reject', 'abstain')),
    approval_order      INTEGER NOT NULL,
    is_required         BOOLEAN NOT NULL DEFAULT TRUE,
    approved_at         TIMESTAMPTZ,
    rejected_at         TIMESTAMPTZ,
    rejection_reason    TEXT,
    expiry_at           TIMESTAMPTZ,
    data_residency_region TEXT NOT NULL DEFAULT 'ng-1' CHECK (data_residency_region IN (
        'ng-1', 'ng-2', 'gh-1', 'ke-1', 'za-1', 'eu-1', 'us-1', 'sg-1'
    )),
    audit_metadata      JSONB DEFAULT '{}'::jsonb,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_signatory_vault_swap ON cryptographic_signatory_vault(swap_record_id);
CREATE INDEX IF NOT EXISTS idx_signatory_vault_status ON cryptographic_signatory_vault(approval_action);
CREATE INDEX IF NOT EXISTS idx_signatory_vault_residency ON cryptographic_signatory_vault(data_residency_region);
CREATE INDEX IF NOT EXISTS idx_signatory_vault_expiry ON cryptographic_signatory_vault(expiry_at) WHERE expiry_at IS NOT NULL;

COMMENT ON TABLE cryptographic_signatory_vault IS 'Multi-sig approval vault for sovereign-tier CBDC token exchanges with regional data residency enforcement';
COMMENT ON COLUMN cryptographic_signatory_vault.signatory_type IS 'Role category of the approving authority';
COMMENT ON COLUMN cryptographic_signatory_vault.signing_algorithm IS 'Cryptographic algorithm used for signature generation (supports PKCS#11 HSM)';
COMMENT ON COLUMN cryptographic_signatory_vault.data_residency_region IS 'Regional data residency zone for sovereign compliance (e.g., ng-1 for Nigeria)';

CREATE TRIGGER update_signatory_vault_updated_at
    BEFORE UPDATE ON cryptographic_signatory_vault
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- migrate:down
DROP TRIGGER IF EXISTS update_signatory_vault_updated_at ON cryptographic_signatory_vault;
DROP TABLE IF EXISTS cryptographic_signatory_vault;
