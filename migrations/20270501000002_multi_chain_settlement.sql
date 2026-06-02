-- Issue #532: Multi-Chain Settlement Interoperability & Cross-EVM Bridges
-- ─────────────────────────────────────────────────────────────────────────────

-- Cross-chain gateway registry
CREATE TABLE IF NOT EXISTS cross_chain_gateways (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_name          TEXT NOT NULL,
    chain_id            BIGINT NOT NULL,               -- EIP-155 chain ID
    rpc_endpoint        TEXT NOT NULL,
    htlc_contract_addr  TEXT,
    min_confirmations   INT  NOT NULL DEFAULT 12,
    enabled             BOOL NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ccg_chain ON cross_chain_gateways (chain_id);

-- Atomic swap escrow lifecycle tracker
CREATE TABLE IF NOT EXISTS atomic_swap_escrows (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL,
    src_chain_id    BIGINT NOT NULL,
    dst_chain_id    BIGINT NOT NULL,
    hashlock        TEXT NOT NULL,                     -- H(secret)
    timelock_expiry TIMESTAMPTZ NOT NULL,
    amount_wei      NUMERIC(40,0) NOT NULL,
    status          TEXT NOT NULL DEFAULT 'INITIATED', -- INITIATED|ASSET_LOCKED|CLAIMED|REFUNDED|TIMED_OUT|HELD_FOR_MANUAL_RECONCILIATION
    src_tx_hash     TEXT,
    dst_tx_hash     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ase_hashlock ON atomic_swap_escrows (hashlock);
CREATE INDEX IF NOT EXISTS idx_ase_status   ON atomic_swap_escrows (status) WHERE status NOT IN ('CLAIMED','REFUNDED');

-- State relay proofs archive with rolling partitions
CREATE TABLE IF NOT EXISTS state_relay_proofs (
    id                  BIGSERIAL,
    swap_id             UUID        NOT NULL,
    chain_id            BIGINT      NOT NULL,
    block_number        BIGINT      NOT NULL,
    state_root          TEXT        NOT NULL,
    merkle_proof        JSONB       NOT NULL,
    gas_cost_wei        NUMERIC(40,0) NOT NULL DEFAULT 0,
    verified            BOOL        NOT NULL DEFAULT FALSE,
    recorded_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, recorded_at)
) PARTITION BY RANGE (recorded_at);

CREATE TABLE IF NOT EXISTS state_relay_proofs_default
    PARTITION OF state_relay_proofs DEFAULT;

CREATE INDEX IF NOT EXISTS idx_srp_swap ON state_relay_proofs (swap_id, recorded_at DESC);
