-- Issue #531: Real-Time Predictive Liquidity Modeling & ML Core
-- ─────────────────────────────────────────────────────────────────────────────

-- TimescaleDB hypertable for liquidity feature vectors (5-minute buckets)
CREATE TABLE IF NOT EXISTS liquidity_time_series_features (
    id              BIGSERIAL,
    corridor_id     UUID        NOT NULL,
    bucket_start    TIMESTAMPTZ NOT NULL,
    throughput_usd  NUMERIC(28,7) NOT NULL DEFAULT 0,
    rolling_variance NUMERIC(28,7) NOT NULL DEFAULT 0,
    velocity_1h     NUMERIC(28,7) NOT NULL DEFAULT 0,
    velocity_24h    NUMERIC(28,7) NOT NULL DEFAULT 0,
    bank_delay_ms   INT         NOT NULL DEFAULT 0,
    PRIMARY KEY (id, bucket_start)
) PARTITION BY RANGE (bucket_start);

CREATE TABLE IF NOT EXISTS liquidity_time_series_features_default
    PARTITION OF liquidity_time_series_features DEFAULT;

CREATE INDEX IF NOT EXISTS idx_ltsf_corridor_bucket
    ON liquidity_time_series_features (corridor_id, bucket_start DESC);

-- ML model registry with versioning and integrity checksums
CREATE TABLE IF NOT EXISTS ml_model_registry (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_name       TEXT NOT NULL,
    version_tag      TEXT NOT NULL,
    artifact_path    TEXT NOT NULL,
    sha256_checksum  TEXT NOT NULL,
    validation_loss  NUMERIC(10,6),
    feature_weights  JSONB,
    deployed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active        BOOL NOT NULL DEFAULT FALSE,
    UNIQUE (model_name, version_tag)
);

-- Inference accuracy log: projected vs actual corridor volumes
CREATE TABLE IF NOT EXISTS inference_accuracy_logs (
    id               BIGSERIAL PRIMARY KEY,
    model_id         UUID        NOT NULL REFERENCES ml_model_registry(id),
    corridor_id      UUID        NOT NULL,
    predicted_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    prediction_window_hours INT NOT NULL DEFAULT 6,
    predicted_volume NUMERIC(28,7) NOT NULL,
    actual_volume    NUMERIC(28,7),
    mae              NUMERIC(28,7),
    confidence_score NUMERIC(5,4),
    triggered_rebalance BOOL NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_ial_model_corridor
    ON inference_accuracy_logs (model_id, corridor_id, predicted_at DESC);
