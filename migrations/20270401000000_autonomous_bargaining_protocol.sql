-- Autonomous Bargaining Protocol schema (Issue #5.03)
-- Tracks negotiation sessions and their full round history.

CREATE TYPE negotiation_state AS ENUM (
    'PROPOSED',
    'COUNTER_OFFER',
    'ACCEPTED',
    'CONTRACT_SIGNED',
    'FAILED'
);

CREATE TABLE negotiation_sessions (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    initiator_id          TEXT NOT NULL,
    responder_id          TEXT NOT NULL,
    state                 negotiation_state NOT NULL DEFAULT 'PROPOSED',
    entrance_payment_ref  TEXT NOT NULL,
    contract_id           TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE negotiation_rounds (
    id           BIGSERIAL PRIMARY KEY,
    session_id   UUID NOT NULL REFERENCES negotiation_sessions(id) ON DELETE CASCADE,
    round        INT  NOT NULL,
    proposer_id  TEXT NOT NULL,
    service_id   TEXT NOT NULL,
    base_price   BIGINT NOT NULL,
    sla_terms    TEXT NOT NULL,
    expiry       TIMESTAMPTZ NOT NULL,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_negotiation_sessions_state ON negotiation_sessions(state);
CREATE INDEX idx_negotiation_rounds_session ON negotiation_rounds(session_id);
