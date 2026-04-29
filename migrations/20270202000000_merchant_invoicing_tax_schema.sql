-- =============================================================================
-- Issue #333: Merchant Invoicing & Automated Tax Calculation
-- =============================================================================

CREATE TABLE merchant_tax_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    region TEXT NOT NULL,
    tax_type TEXT NOT NULL,
    rate_bps INTEGER NOT NULL CHECK (rate_bps >= 0 AND rate_bps <= 10000),
    is_inclusive BOOLEAN NOT NULL DEFAULT FALSE,
    applies_to TEXT[] NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    effective_from TIMESTAMPTZ NOT NULL DEFAULT now(),
    effective_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE merchant_tax_rules IS 'Dynamic tax rule configurations per merchant and region.';
COMMENT ON COLUMN merchant_tax_rules.rate_bps IS 'Tax rate in basis points (750 = 7.5% Nigerian VAT).';

CREATE TABLE merchant_invoices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invoice_number TEXT NOT NULL UNIQUE,
    transaction_id UUID REFERENCES transactions(transaction_id) ON DELETE SET NULL,
    wallet_address VARCHAR(255),
    customer_profile_id UUID,
    subtotal NUMERIC(36, 18) NOT NULL CHECK (subtotal >= 0),
    tax_amount NUMERIC(36, 18) NOT NULL DEFAULT 0 CHECK (tax_amount >= 0),
    total_amount NUMERIC(36, 18) NOT NULL CHECK (total_amount >= 0),
    currency TEXT NOT NULL DEFAULT 'cNGN',
    tax_breakdown JSONB NOT NULL DEFAULT '[]',
    line_items JSONB NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'issued', 'paid', 'void', 'refunded')),
    content_hash TEXT,
    digital_signature TEXT,
    pdf_storage_key TEXT,
    qr_code_data TEXT,
    accounting_sync_status TEXT DEFAULT 'pending' CHECK (accounting_sync_status IN ('pending', 'synced', 'failed', 'skipped')),
    accounting_external_id TEXT,
    accounting_platform TEXT,
    issued_at TIMESTAMPTZ,
    due_at TIMESTAMPTZ,
    paid_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE merchant_invoices IS 'Cryptographically signed digital invoices per transaction.';

CREATE TABLE merchant_tax_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    report_period_start DATE NOT NULL,
    report_period_end DATE NOT NULL,
    total_gross_revenue NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_tax_collected NUMERIC(36, 18) NOT NULL DEFAULT 0,
    total_net_revenue NUMERIC(36, 18) NOT NULL DEFAULT 0,
    invoice_count INTEGER NOT NULL DEFAULT 0,
    tax_breakdown_by_type JSONB NOT NULL DEFAULT '{}',
    attestation_hash TEXT,
    attestation_generated_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'finalized', 'submitted')),
    submitted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, report_period_start, report_period_end)
);

COMMENT ON TABLE merchant_tax_reports IS 'Monthly tax collection reports formatted for FIRS submission.';

CREATE TABLE merchant_accounting_integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    credentials_encrypted TEXT NOT NULL,
    field_mapping JSONB NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    last_sync_at TIMESTAMPTZ,
    last_sync_status TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, platform)
);

COMMENT ON TABLE merchant_accounting_integrations IS 'Accounting software connector configurations per merchant.';

CREATE INDEX idx_mi_merchant_id ON merchant_invoices(merchant_id);
CREATE INDEX idx_mi_transaction_id ON merchant_invoices(transaction_id);
CREATE INDEX idx_mi_status ON merchant_invoices(status);
CREATE INDEX idx_mi_issued_at ON merchant_invoices(issued_at);
CREATE INDEX idx_mtr_merchant_period ON merchant_tax_reports(merchant_id, report_period_start);
CREATE INDEX idx_mtr_merchant_id ON merchant_tax_rules(merchant_id);

CREATE TRIGGER set_updated_at_merchant_tax_rules
    BEFORE UPDATE ON merchant_tax_rules
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_merchant_invoices
    BEFORE UPDATE ON merchant_invoices
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_merchant_tax_reports
    BEFORE UPDATE ON merchant_tax_reports
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_merchant_accounting_integrations
    BEFORE UPDATE ON merchant_accounting_integrations
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
