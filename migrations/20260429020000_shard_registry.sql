-- migrate:up
-- Database sharding infrastructure (Issue #423).
-- The shard_registry lives on the PRIMARY (coordinator) database.
-- Each shard is a separate PostgreSQL instance; this table tells the
-- application router which shard owns which key range.

CREATE TABLE shard_registry (
    shard_id     SMALLINT    PRIMARY KEY,
    dsn          TEXT        NOT NULL,          -- connection string for this shard
    status       TEXT        NOT NULL DEFAULT 'active'
                             CHECK (status IN ('active', 'draining', 'offline')),
    -- Consistent-hash ring: this shard owns keys where
    --   hash(key) % total_shards == shard_id
    -- These columns are informational; routing logic lives in the app.
    weight       SMALLINT    NOT NULL DEFAULT 1, -- relative ring weight
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE shard_registry IS
    'Registry of all database shards. Read by ShardRouter on startup and on SIGHUP.';

-- Seed with a single shard pointing at the primary DB (DATABASE_URL).
-- Add rows here to introduce new shards; the router hot-reloads on change.
INSERT INTO shard_registry (shard_id, dsn, status)
VALUES (0, current_setting('app.primary_dsn', true), 'active')
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Shard migration tracking
-- Records every row-level migration job so the operator can monitor progress
-- and resume after a crash without re-processing already-moved rows.
-- ---------------------------------------------------------------------------

CREATE TABLE shard_migration_jobs (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    table_name      TEXT        NOT NULL,
    source_shard    SMALLINT    NOT NULL REFERENCES shard_registry(shard_id),
    target_shard    SMALLINT    NOT NULL REFERENCES shard_registry(shard_id),
    shard_key_col   TEXT        NOT NULL,   -- e.g. "wallet_address"
    last_key        TEXT,                   -- cursor: last migrated key value
    rows_migrated   BIGINT      NOT NULL DEFAULT 0,
    status          TEXT        NOT NULL DEFAULT 'pending'
                                CHECK (status IN ('pending','running','done','failed')),
    error_message   TEXT,
    started_at      TIMESTAMPTZ,
    finished_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_shard_migration_status ON shard_migration_jobs(status);

CREATE TRIGGER set_updated_at_shard_registry
    BEFORE UPDATE ON shard_registry
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_shard_migration_jobs
    BEFORE UPDATE ON shard_migration_jobs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- migrate:down
DROP TABLE IF EXISTS shard_migration_jobs;
DROP TABLE IF EXISTS shard_registry;
