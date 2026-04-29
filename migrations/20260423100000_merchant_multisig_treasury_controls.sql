-- Migration: Merchant Multi-Sig & Treasury Controls — Issue #336
--
-- Tables (in dependency order):
--   1. merchant_signing_groups
--   2. merchant_signing_group_members
--   3. merchant_signing_policies  (references groups)
--   4. merchant_proposals         (references policies)
--   5. merchant_proposal_signatures (references proposals)
--   6. merchant_freeze_state

-- ── 1. Signing Groups ─────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS merchant_signing_groups (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id TEXT        NOT NULL,
    group_name  TEXT        NOT NULL,
    description TEXT,
    created_by  TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_merchant_signing_groups_merchant
    ON merchant_signing_groups (merchant_id);

-- ── 2. Signing Group Members ──────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS merchant_signing_group_members (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id    UUID        NOT NULL REFERENCES merchant_signing_groups(id) ON DELETE CASCADE,
    signer_id   TEXT        NOT NULL,
    signer_name TEXT        NOT NULL,
    signer_role TEXT        NOT NULL,
    is_active   BOOLEAN     NOT NULL DEFAULT true,
    added_by    TEXT        NOT NULL,
    added_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (group_id, signer_id)
);
CREATE INDEX IF NOT EXISTS idx_merchant_signing_group_members_group
    ON merchant_signing_group_members (group_id, is_active);

-- ── 3. Signing Policies ───────────────────────────────────────────────────────
-- "Any payout > 5,000,000 cNGN requires 3 of 5 executive signatures"
CREATE TABLE IF NOT EXISTS merchant_signing_policies (
    id                   UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id          TEXT        NOT NULL,
    policy_name          TEXT        NOT NULL,
    -- 'payout' | 'api_key_update' | 'tax_config_update' | 'any'
    action_type          TEXT        NOT NULL DEFAULT 'any',
    -- Minimum cNGN amount that triggers this policy; NULL = always triggers
    high_value_threshold NUMERIC(36, 8),
    required_signatures  INT         NOT NULL CHECK (required_signatures >= 1),
    total_signers        INT         NOT NULL CHECK (total_signers >= required_signatures),
    signing_group_id     UUID        REFERENCES merchant_signing_groups(id) ON DELETE SET NULL,
    is_active            BOOLEAN     NOT NULL DEFAULT true,
    created_by           TEXT        NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_merchant_signing_policies_merchant
    ON merchant_signing_policies (merchant_id, is_active);

-- ── 4. Proposals ──────────────────────────────────────────────────────────────
-- A proposed high-value action awaiting multi-sig approval.
CREATE TABLE IF NOT EXISTS merchant_proposals (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id      TEXT        NOT NULL,
    policy_id        UUID        NOT NULL REFERENCES merchant_signing_policies(id),
    action_type      TEXT        NOT NULL,
    action_payload   JSONB       NOT NULL DEFAULT '{}'::jsonb,
    amount           NUMERIC(36, 8),
    -- 'pending' | 'approved' | 'rejected' | 'expired' | 'executed'
    status           TEXT        NOT NULL DEFAULT 'pending',
    proposed_by      TEXT        NOT NULL,
    proposed_by_name TEXT        NOT NULL,
    expires_at       TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '24 hours'),
    approved_at      TIMESTAMPTZ,
    executed_at      TIMESTAMPTZ,
    rejected_at      TIMESTAMPTZ,
    rejection_reason TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_merchant_proposals_merchant_status
    ON merchant_proposals (merchant_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_merchant_proposals_expires
    ON merchant_proposals (expires_at) WHERE status = 'pending';

-- ── 5. Proposal Signatures ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS merchant_proposal_signatures (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    proposal_id UUID        NOT NULL REFERENCES merchant_proposals(id) ON DELETE CASCADE,
    signer_id   TEXT        NOT NULL,
    signer_name TEXT        NOT NULL,
    signer_role TEXT        NOT NULL,
    decision    TEXT        NOT NULL CHECK (decision IN ('approved', 'rejected')),
    comment     TEXT,
    signed_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (proposal_id, signer_id)
);
CREATE INDEX IF NOT EXISTS idx_merchant_proposal_signatures_proposal
    ON merchant_proposal_signatures (proposal_id);

-- ── 6. Freeze State ───────────────────────────────────────────────────────────
-- Emergency 1-of-N freeze: CEO / Security Officer locks all outgoing funds.
CREATE TABLE IF NOT EXISTS merchant_freeze_state (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id     TEXT        NOT NULL UNIQUE,
    is_frozen       BOOLEAN     NOT NULL DEFAULT false,
    frozen_by       TEXT,
    frozen_by_name  TEXT,
    freeze_reason   TEXT,
    frozen_at       TIMESTAMPTZ,
    unfrozen_by     TEXT,
    unfrozen_at     TIMESTAMPTZ,
    unfreeze_reason TEXT,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_merchant_freeze_state_frozen
    ON merchant_freeze_state (is_frozen) WHERE is_frozen = true;
