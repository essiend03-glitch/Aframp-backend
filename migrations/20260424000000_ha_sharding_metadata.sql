-- Migration: HA sharding metadata (Issue #347)
-- Tracks logical shard topology so the application can discover shards
-- at runtime and support hot shard addition without a restart.

CREATE TABLE IF NOT EXISTS ha_shards (
    shard_id        INTEGER PRIMARY KEY,
    primary_url     TEXT        NOT NULL,
    replica_urls    TEXT[]      NOT NULL DEFAULT '{}',
    is_active       BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Tracks per-replica health history for observability / alerting
CREATE TABLE IF NOT EXISTS ha_replica_health (
    id              BIGSERIAL   PRIMARY KEY,
    shard_id        INTEGER     NOT NULL REFERENCES ha_shards(shard_id),
    replica_index   SMALLINT    NOT NULL,
    is_healthy      BOOLEAN     NOT NULL,
    checksum        BIGINT,
    checked_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ha_replica_health_shard_time
    ON ha_replica_health (shard_id, checked_at DESC);

-- Seed initial shard 0 (single-node bootstrap; override via env / Patroni)
INSERT INTO ha_shards (shard_id, primary_url, replica_urls)
VALUES (0, current_setting('app.primary_url', true), '{}')
ON CONFLICT (shard_id) DO NOTHING;
