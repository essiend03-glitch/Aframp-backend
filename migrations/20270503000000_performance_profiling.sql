-- Performance Profiling Infrastructure
-- Issue: Performance profiling and monitoring

-- Performance profiles table - stores trace execution tallies and slow-endpoint summaries
CREATE TABLE performance_profiles (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    endpoint_path           TEXT NOT NULL,
    method                  TEXT NOT NULL,
    p50_duration_ms         REAL,
    p95_duration_ms         REAL,
    p99_duration_ms         REAL,
    avg_duration_ms         REAL,
    max_duration_ms         REAL,
    min_duration_ms         REAL,
    request_count           BIGINT NOT NULL DEFAULT 0,
    error_count             BIGINT NOT NULL DEFAULT 0,
    slow_request_count      BIGINT NOT NULL DEFAULT 0,
    memory_allocated_bytes  BIGINT,
    memory_peak_bytes       BIGINT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Optimized indexes for fast trend analysis
CREATE INDEX idx_perf_profiles_endpoint ON performance_profiles (endpoint_path, p99_duration_ms, created_at);
CREATE INDEX idx_perf_profiles_p95 ON performance_profiles (p95_duration_ms, created_at DESC);
CREATE INDEX idx_perf_profiles_p99 ON performance_profiles (p99_duration_ms, created_at DESC);
CREATE INDEX idx_perf_profiles_updated ON performance_profiles (updated_at DESC);

-- Memory allocation snapshots
CREATE TABLE memory_snapshots (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    endpoint_path           TEXT NOT NULL,
    allocation_count        BIGINT NOT NULL DEFAULT 0,
    total_bytes_allocated   BIGINT NOT NULL DEFAULT 0,
    total_bytes_deallocated BIGINT NOT NULL DEFAULT 0,
    peak_bytes_allocated    BIGINT NOT NULL DEFAULT 0,
    avg_allocation_bytes    REAL,
    vector_reallocs         BIGINT NOT NULL DEFAULT 0,
    snapshot_time           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_memory_snapshots_endpoint ON memory_snapshots (endpoint_path, snapshot_time DESC);

-- Trace execution tallies
CREATE TABLE trace_tallies (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trace_id                TEXT NOT NULL,
    span_name               TEXT NOT NULL,
    parent_span_id          TEXT,
    endpoint_path           TEXT NOT NULL,
    start_time              TIMESTAMPTZ NOT NULL,
    end_time                TIMESTAMPTZ,
    duration_ms             REAL,
    poll_count              INTEGER,
    poll_duration_ms        REAL,
    blocked_duration_ms     REAL,
    scheduled_count         INTEGER,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_trace_tallies_trace ON trace_tallies (trace_id);
CREATE INDEX idx_trace_tallies_endpoint ON trace_tallies (endpoint_path, created_at DESC);

-- Profiling configuration and sample rates
CREATE TABLE profiling_config (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sample_rate             REAL NOT NULL DEFAULT 0.01, -- 1% default
    slow_request_threshold_ms REAL NOT NULL DEFAULT 100,
    enable_memory_profiling BOOLEAN NOT NULL DEFAULT FALSE,
    enable_trace_collection BOOLEAN NOT NULL DEFAULT TRUE,
    max_traces_per_minute   INTEGER NOT NULL DEFAULT 1000,
    p95_threshold_ms        REAL NOT NULL DEFAULT 25.0,
    p99_threshold_ms        REAL NOT NULL DEFAULT 100.0,
    is_active               BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by              UUID
);

-- Insert default configuration
INSERT INTO profiling_config (sample_rate, slow_request_threshold_ms, p95_threshold_ms, p99_threshold_ms)
VALUES (0.01, 100, 25.0, 100.0);

-- Slow endpoint alerts log
CREATE TABLE slow_endpoint_alerts (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    endpoint_path           TEXT NOT NULL,
    method                  TEXT NOT NULL,
    latency_p95_ms          REAL,
    latency_p99_ms          REAL,
    alert_threshold_ms      REAL,
    alert_type              TEXT NOT NULL, -- 'p95_exceeded', 'p99_exceeded'
    alert_severity          TEXT NOT NULL, -- 'warning', 'critical'
    triggered_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged            BOOLEAN NOT NULL DEFAULT FALSE,
    acknowledged_by         UUID,
    acknowledged_at         TIMESTAMPTZ
);

CREATE INDEX idx_slow_alerts_endpoint ON slow_endpoint_alerts (endpoint_path, triggered_at DESC);
CREATE INDEX idx_slow_alerts_unack ON slow_endpoint_alerts (acknowledged, triggered_at DESC);