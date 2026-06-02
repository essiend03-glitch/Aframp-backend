-- migrate:up
-- Replication lag audit log (Issue #348).
-- Stores a time-series of measured replication lag per replica so operators
-- can query historical lag trends and set up alerting rules.

CREATE TABLE IF NOT EXISTS replication_lag_log (
    id              BIGSERIAL   PRIMARY KEY,
    replica_label   TEXT        NOT NULL,           -- e.g. "eu-west-1"
    lag_ms          INTEGER     NOT NULL,            -- measured lag in milliseconds
    breaker_open    BOOLEAN     NOT NULL DEFAULT FALSE,
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Efficient time-range queries per replica
CREATE INDEX IF NOT EXISTS idx_replication_lag_log_replica_time
    ON replication_lag_log (replica_label, recorded_at DESC);

-- Retention: rows older than 7 days are pruned by the maintenance worker.
COMMENT ON TABLE replication_lag_log IS
    'Time-series of replication lag measurements. Pruned after 7 days.';

-- migrate:down
DROP TABLE IF EXISTS replication_lag_log;
