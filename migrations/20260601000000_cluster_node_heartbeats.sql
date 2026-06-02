-- migrate:up
-- Cluster node heartbeat coordination table (Issue #456).
-- Tracks active, dynamically scaling stateless application node instances,
-- their assigned internal IP addresses, and operational health metrics.
-- Used for horizontal scaling coordination across cNGN transaction workloads.

CREATE TABLE cluster_node_heartbeats (
    node_id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    internal_ip     INET        NOT NULL,
    region          TEXT        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'healthy'
                                CHECK (status IN ('healthy', 'degraded', 'draining', 'offline')),
    cpu_usage_pct   SMALLINT    CHECK (cpu_usage_pct BETWEEN 0 AND 100),
    mem_usage_pct   SMALLINT    CHECK (mem_usage_pct BETWEEN 0 AND 100),
    active_conns    INT         NOT NULL DEFAULT 0,
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    registered_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE cluster_node_heartbeats IS
    'Short-lived registry of active application nodes for stateless horizontal scaling coordination. '
    'Rows older than 60 s are considered stale and eligible for pruning.';

-- Primary lookup: find live nodes quickly by recency.
CREATE INDEX idx_cluster_node_heartbeats_last_seen
    ON cluster_node_heartbeats (last_seen_at DESC);

-- Secondary: filter by region and status for regional health checks.
CREATE INDEX idx_cluster_node_heartbeats_region_status
    ON cluster_node_heartbeats (region, status);

-- Automatic pruning: remove stale heartbeats older than 60 seconds.
-- Invoked by the background maintenance worker; kept lightweight via the index above.
CREATE OR REPLACE FUNCTION prune_stale_cluster_nodes() RETURNS void
    LANGUAGE sql AS
$$
    DELETE FROM cluster_node_heartbeats
    WHERE last_seen_at < now() - INTERVAL '60 seconds';
$$;

COMMENT ON FUNCTION prune_stale_cluster_nodes() IS
    'Removes heartbeat rows not updated within the last 60 s. '
    'Call periodically from the maintenance worker.';

-- migrate:down
DROP FUNCTION IF EXISTS prune_stale_cluster_nodes();
DROP TABLE IF EXISTS cluster_node_heartbeats;
