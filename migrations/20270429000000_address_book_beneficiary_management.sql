-- Address Book and Beneficiary Management System
-- Migration: 20270429000000_address_book_beneficiary_management.sql

-- Address book entries table
CREATE TABLE IF NOT EXISTS address_book_entries (
    id UUID PRIMARY KEY,
    owner_wallet_id UUID NOT NULL REFERENCES wallet_registry(id) ON DELETE CASCADE,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('stellar-wallet', 'mobile-money', 'bank-account')),
    label TEXT NOT NULL,
    notes TEXT,
    entry_status TEXT NOT NULL DEFAULT 'active' CHECK (entry_status IN ('active', 'deleted')),
    verification_status TEXT NOT NULL DEFAULT 'pending' CHECK (verification_status IN ('verified', 'pending', 'failed', 'stale', 'not-supported')),
    last_used_at TIMESTAMPTZ,
    use_count INTEGER NOT NULL DEFAULT 0,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Stellar wallet entry details
CREATE TABLE IF NOT EXISTS stellar_wallet_entries (
    entry_id UUID PRIMARY KEY REFERENCES address_book_entries(id) ON DELETE CASCADE,
    stellar_public_key TEXT NOT NULL,
    network TEXT NOT NULL CHECK (network IN ('testnet', 'mainnet')),
    account_exists_on_stellar BOOLEAN NOT NULL DEFAULT FALSE,
    cngn_trustline_active BOOLEAN NOT NULL DEFAULT FALSE,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Mobile money entry details
CREATE TABLE IF NOT EXISTS mobile_money_entries (
    entry_id UUID PRIMARY KEY REFERENCES address_book_entries(id) ON DELETE CASCADE,
    provider_name TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    account_name TEXT,
    country_code TEXT NOT NULL,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Bank account entry details
CREATE TABLE IF NOT EXISTS bank_account_entries (
    entry_id UUID PRIMARY KEY REFERENCES address_book_entries(id) ON DELETE CASCADE,
    bank_name TEXT NOT NULL,
    account_number TEXT NOT NULL,
    account_name TEXT,
    sort_code TEXT,
    routing_number TEXT,
    country_code TEXT NOT NULL,
    currency TEXT NOT NULL,
    last_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Address groups
CREATE TABLE IF NOT EXISTS address_groups (
    id UUID PRIMARY KEY,
    owner_wallet_id UUID NOT NULL REFERENCES wallet_registry(id) ON DELETE CASCADE,
    group_name TEXT NOT NULL,
    group_description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_wallet_id, group_name)
);

-- Group memberships
CREATE TABLE IF NOT EXISTS group_memberships (
    group_id UUID NOT NULL REFERENCES address_groups(id) ON DELETE CASCADE,
    entry_id UUID NOT NULL REFERENCES address_book_entries(id) ON DELETE CASCADE,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (group_id, entry_id)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_address_book_entries_owner ON address_book_entries(owner_wallet_id) WHERE entry_status = 'active';
CREATE INDEX IF NOT EXISTS idx_address_book_entries_type ON address_book_entries(entry_type) WHERE entry_status = 'active';
CREATE INDEX IF NOT EXISTS idx_address_book_entries_verification ON address_book_entries(verification_status) WHERE entry_status = 'active';
CREATE INDEX IF NOT EXISTS idx_address_book_entries_last_used ON address_book_entries(last_used_at DESC NULLS LAST) WHERE entry_status = 'active';
CREATE INDEX IF NOT EXISTS idx_address_book_entries_use_count ON address_book_entries(use_count DESC) WHERE entry_status = 'active';
CREATE INDEX IF NOT EXISTS idx_address_book_entries_deleted ON address_book_entries(deleted_at) WHERE entry_status = 'deleted';
CREATE INDEX IF NOT EXISTS idx_address_book_entries_label_search ON address_book_entries USING gin(to_tsvector('english', label)) WHERE entry_status = 'active';
CREATE INDEX IF NOT EXISTS idx_address_book_entries_notes_search ON address_book_entries USING gin(to_tsvector('english', notes)) WHERE entry_status = 'active' AND notes IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_stellar_wallet_entries_public_key ON stellar_wallet_entries(stellar_public_key);
CREATE INDEX IF NOT EXISTS idx_stellar_wallet_entries_verification ON stellar_wallet_entries(last_verified_at);

CREATE INDEX IF NOT EXISTS idx_mobile_money_entries_phone ON mobile_money_entries(phone_number);
CREATE INDEX IF NOT EXISTS idx_mobile_money_entries_provider ON mobile_money_entries(provider_name);

CREATE INDEX IF NOT EXISTS idx_bank_account_entries_account_number ON bank_account_entries(account_number);
CREATE INDEX IF NOT EXISTS idx_bank_account_entries_bank ON bank_account_entries(bank_name);

CREATE INDEX IF NOT EXISTS idx_address_groups_owner ON address_groups(owner_wallet_id);
CREATE INDEX IF NOT EXISTS idx_group_memberships_entry ON group_memberships(entry_id);

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_address_book_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER address_book_entries_updated_at
    BEFORE UPDATE ON address_book_entries
    FOR EACH ROW
    EXECUTE FUNCTION update_address_book_updated_at();

CREATE TRIGGER stellar_wallet_entries_updated_at
    BEFORE UPDATE ON stellar_wallet_entries
    FOR EACH ROW
    EXECUTE FUNCTION update_address_book_updated_at();

CREATE TRIGGER mobile_money_entries_updated_at
    BEFORE UPDATE ON mobile_money_entries
    FOR EACH ROW
    EXECUTE FUNCTION update_address_book_updated_at();

CREATE TRIGGER bank_account_entries_updated_at
    BEFORE UPDATE ON bank_account_entries
    FOR EACH ROW
    EXECUTE FUNCTION update_address_book_updated_at();

CREATE TRIGGER address_groups_updated_at
    BEFORE UPDATE ON address_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_address_book_updated_at();

-- Cleanup job for permanently deleting soft-deleted entries after 30 days
-- This should be run by a background worker
CREATE OR REPLACE FUNCTION cleanup_deleted_address_book_entries()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    WITH deleted AS (
        DELETE FROM address_book_entries
        WHERE entry_status = 'deleted'
        AND deleted_at < NOW() - INTERVAL '30 days'
        RETURNING id
    )
    SELECT COUNT(*) INTO deleted_count FROM deleted;
    
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Function to mark stale verifications
CREATE OR REPLACE FUNCTION mark_stale_verifications(stale_threshold_hours INTEGER DEFAULT 168)
RETURNS INTEGER AS $$
DECLARE
    updated_count INTEGER;
BEGIN
    WITH updated AS (
        UPDATE address_book_entries
        SET verification_status = 'stale'
        WHERE entry_status = 'active'
        AND verification_status = 'verified'
        AND updated_at < NOW() - (stale_threshold_hours || ' hours')::INTERVAL
        RETURNING id
    )
    SELECT COUNT(*) INTO updated_count FROM updated;
    
    RETURN updated_count;
END;
$$ LANGUAGE plpgsql;

-- Comments for documentation
COMMENT ON TABLE address_book_entries IS 'Main address book entries for all wallet types';
COMMENT ON TABLE stellar_wallet_entries IS 'Stellar wallet address details with verification status';
COMMENT ON TABLE mobile_money_entries IS 'Mobile money account details for offramp destinations';
COMMENT ON TABLE bank_account_entries IS 'Bank account details for offramp destinations';
COMMENT ON TABLE address_groups IS 'User-defined groups for organizing address book entries';
COMMENT ON TABLE group_memberships IS 'Many-to-many relationship between groups and entries';

COMMENT ON COLUMN address_book_entries.entry_status IS 'Active or soft-deleted status';
COMMENT ON COLUMN address_book_entries.verification_status IS 'Verification state: verified, pending, failed, stale, not-supported';
COMMENT ON COLUMN address_book_entries.use_count IS 'Number of times this entry has been used in transactions';
COMMENT ON COLUMN address_book_entries.deleted_at IS 'Timestamp of soft deletion, entries are permanently deleted after 30 days';

COMMENT ON COLUMN stellar_wallet_entries.account_exists_on_stellar IS 'Whether the account exists on the Stellar network';
COMMENT ON COLUMN stellar_wallet_entries.cngn_trustline_active IS 'Whether the account has an active cNGN trustline';

COMMENT ON FUNCTION cleanup_deleted_address_book_entries() IS 'Permanently delete soft-deleted entries older than 30 days';
COMMENT ON FUNCTION mark_stale_verifications(INTEGER) IS 'Mark verified entries as stale if not re-verified within threshold';
