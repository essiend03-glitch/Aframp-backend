-- Banking Partner Integration & Account Linkage (Issue #407)
-- Tables: linked_bank_accounts, bank_mandates, bank_reconciliation_runs, bank_webhook_events

-- Linked bank accounts (tokenized — no plaintext credentials stored)
CREATE TABLE IF NOT EXISTS linked_bank_accounts (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Tokenized reference returned by the bank/payment provider (never raw account number)
    account_token       TEXT NOT NULL UNIQUE,
    -- Masked display value e.g. "****1234"
    account_mask        TEXT NOT NULL,
    account_name        TEXT NOT NULL,
    bank_code           TEXT NOT NULL,
    bank_name           TEXT NOT NULL,
    currency            CHAR(3) NOT NULL DEFAULT 'NGN',
    -- 'active' | 'suspended' | 'unlinked'
    status              TEXT NOT NULL DEFAULT 'active',
    -- BVN/NIN hash (SHA-256, never plaintext)
    identity_hash       TEXT,
    -- Provider that verified this account: 'flutterwave' | 'paystack' | 'nibss'
    verified_by         TEXT NOT NULL,
    verified_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_linked_bank_accounts_user_id ON linked_bank_accounts(user_id);
CREATE INDEX idx_linked_bank_accounts_status  ON linked_bank_accounts(status);

-- Direct debit / credit mandates
CREATE TABLE IF NOT EXISTS bank_mandates (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    linked_account_id   UUID NOT NULL REFERENCES linked_bank_accounts(id) ON DELETE CASCADE,
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- 'debit' | 'credit'
    mandate_type        TEXT NOT NULL,
    -- 'active' | 'revoked' | 'expired'
    status              TEXT NOT NULL DEFAULT 'active',
    -- Maximum single-transaction amount in minor units (kobo)
    max_amount          BIGINT NOT NULL,
    -- Provider mandate reference (e.g. Paystack authorization code)
    provider_reference  TEXT NOT NULL UNIQUE,
    provider            TEXT NOT NULL,
    expires_at          TIMESTAMPTZ,
    revoked_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bank_mandates_linked_account ON bank_mandates(linked_account_id);
CREATE INDEX idx_bank_mandates_user_id        ON bank_mandates(user_id);
CREATE INDEX idx_bank_mandates_status         ON bank_mandates(status);

-- Idempotent transfer log — prevents double-charging on network retries
CREATE TABLE IF NOT EXISTS bank_transfer_log (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    idempotency_key     TEXT NOT NULL UNIQUE,
    mandate_id          UUID REFERENCES bank_mandates(id),
    linked_account_id   UUID NOT NULL REFERENCES linked_bank_accounts(id),
    -- 'debit' | 'credit'
    direction           TEXT NOT NULL,
    amount              BIGINT NOT NULL,
    currency            CHAR(3) NOT NULL DEFAULT 'NGN',
    -- 'pending' | 'success' | 'failed' | 'reversed'
    status              TEXT NOT NULL DEFAULT 'pending',
    provider            TEXT NOT NULL,
    provider_reference  TEXT,
    provider_response   JSONB,
    failure_reason      TEXT,
    settled_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bank_transfer_log_idempotency ON bank_transfer_log(idempotency_key);
CREATE INDEX idx_bank_transfer_log_account     ON bank_transfer_log(linked_account_id);
CREATE INDEX idx_bank_transfer_log_status      ON bank_transfer_log(status);

-- Daily reconciliation runs comparing Aframp ledger vs bank EOD statements
CREATE TABLE IF NOT EXISTS bank_reconciliation_runs (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_date            DATE NOT NULL,
    bank_code           TEXT NOT NULL,
    -- 'equilibrium' | 'discrepancy' | 'pending_review'
    status              TEXT NOT NULL DEFAULT 'pending_review',
    aframp_total        NUMERIC(20, 8) NOT NULL DEFAULT 0,
    bank_total          NUMERIC(20, 8) NOT NULL DEFAULT 0,
    discrepancy         NUMERIC(20, 8) NOT NULL DEFAULT 0,
    flagged_count       INT NOT NULL DEFAULT 0,
    metadata            JSONB,
    reviewed_by         UUID REFERENCES users(id),
    reviewed_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (run_date, bank_code)
);

CREATE INDEX idx_bank_recon_runs_date   ON bank_reconciliation_runs(run_date);
CREATE INDEX idx_bank_recon_runs_status ON bank_reconciliation_runs(status);

-- Inbound bank webhook events (raw payload preserved for replay/audit)
CREATE TABLE IF NOT EXISTS bank_webhook_events (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider            TEXT NOT NULL,
    event_type          TEXT NOT NULL,
    -- Provider's own event ID for deduplication
    provider_event_id   TEXT NOT NULL,
    payload             JSONB NOT NULL,
    -- 'received' | 'processed' | 'failed' | 'ignored'
    status              TEXT NOT NULL DEFAULT 'received',
    linked_account_id   UUID REFERENCES linked_bank_accounts(id),
    transfer_log_id     UUID REFERENCES bank_transfer_log(id),
    error_message       TEXT,
    processed_at        TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider, provider_event_id)
);

CREATE INDEX idx_bank_webhook_events_provider ON bank_webhook_events(provider, event_type);
CREATE INDEX idx_bank_webhook_events_status   ON bank_webhook_events(status);
