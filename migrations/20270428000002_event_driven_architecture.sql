-- Migration: Event-Driven Architecture Schema (Issue #399)
-- Durable event store, dead-letter queue, and idempotency table

CREATE TYPE event_delivery_status AS ENUM (
    'pending',
    'delivered',
    'failed',
    'dead_lettered'
);

-- Immutable event store — every published event is persisted here
-- Enables audit trail and event replay for recovery
CREATE TABLE event_records (
    event_id        UUID PRIMARY KEY,
    event_type      TEXT NOT NULL,
    aggregate_id    TEXT NOT NULL,
    aggregate_type  TEXT NOT NULL,
    payload         JSONB NOT NULL DEFAULT '{}',
    metadata        JSONB NOT NULL DEFAULT '{}',
    schema_version  TEXT NOT NULL DEFAULT '1.0',
    published_at    TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_event_records_event_type ON event_records (event_type);
CREATE INDEX idx_event_records_aggregate_id ON event_records (aggregate_id);
CREATE INDEX idx_event_records_published_at ON event_records (published_at DESC);

-- Dead-letter queue — events that failed processing after max retries
CREATE TABLE dead_letter_queue (
    dlq_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id            UUID NOT NULL REFERENCES event_records (event_id),
    consumer_name       TEXT NOT NULL,
    failure_reason      TEXT NOT NULL,
    retry_count         INT NOT NULL DEFAULT 1,
    last_attempted_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (event_id, consumer_name)
);

CREATE INDEX idx_dlq_consumer_name ON dead_letter_queue (consumer_name);

-- Idempotency table — prevents duplicate side-effects from at-least-once delivery
CREATE TABLE processed_events (
    event_id        UUID NOT NULL,
    consumer_name   TEXT NOT NULL,
    processed_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (event_id, consumer_name)
);

-- Event subscriptions registry
CREATE TABLE event_subscriptions (
    subscription_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    consumer_name   TEXT NOT NULL UNIQUE,
    event_types     TEXT[] NOT NULL DEFAULT '{}',
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
