-- Capacity Planning Schema — Issue #463
-- Provisions two tables for the predictive capacity planning pipeline:
--   1. infrastructure_capacity_snapshots — point-in-time resource utilisation readings
--   2. growth_forecast_models            — regression parameters and depletion predictions

-- ── 1. Infrastructure Capacity Snapshots ─────────────────────────────────────
-- Stores periodic snapshots of actual infrastructure resource utilisation.
-- Each row is one observation window (e.g. hourly or daily) per host/cluster.

CREATE TABLE IF NOT EXISTS infrastructure_capacity_snapshots (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),

    -- When and where
    snapshot_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    region                  TEXT        NOT NULL DEFAULT 'us-east-1',
    host_or_cluster         TEXT        NOT NULL,           -- e.g. "db-primary", "api-cluster"
    resource_type           TEXT        NOT NULL            -- "compute" | "memory" | "storage" | "db_io" | "network_io" | "ledger"
                                CHECK (resource_type IN ('compute','memory','storage','db_io','network_io','ledger')),

    -- Utilisation metrics (all nullable — not every resource type has every field)
    cpu_utilisation_pct     NUMERIC(6,3),                   -- 0–100
    memory_used_gb          NUMERIC(12,3),
    memory_total_gb         NUMERIC(12,3),
    storage_used_gb         NUMERIC(16,3),
    storage_total_gb        NUMERIC(16,3),
    storage_iops            BIGINT,
    network_rx_mbps         NUMERIC(12,3),
    network_tx_mbps         NUMERIC(12,3),
    db_connections_active   INT,
    db_connections_max      INT,
    db_query_p95_ms         NUMERIC(10,3),
    ledger_volume_ngn       NUMERIC(24,6),                  -- cNGN ledger value in NGN
    transaction_velocity    NUMERIC(12,3),                  -- TPS at snapshot time

    -- Derived saturation ratio (used_/total_) — pre-computed for fast alerting queries
    saturation_ratio        NUMERIC(6,4) GENERATED ALWAYS AS (
        CASE
            WHEN resource_type = 'storage'  AND storage_total_gb  > 0
                THEN storage_used_gb  / storage_total_gb
            WHEN resource_type = 'memory'   AND memory_total_gb   > 0
                THEN memory_used_gb   / memory_total_gb
            WHEN resource_type = 'db_io'    AND db_connections_max > 0
                THEN db_connections_active::NUMERIC / db_connections_max
            ELSE NULL
        END
    ) STORED,

    -- Collection metadata
    collector               TEXT        NOT NULL DEFAULT 'aframp-metrics-agent',
    raw_payload             JSONB       NOT NULL DEFAULT '{}',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ics_snapshot_at
    ON infrastructure_capacity_snapshots (snapshot_at DESC);
CREATE INDEX IF NOT EXISTS idx_ics_host_resource
    ON infrastructure_capacity_snapshots (host_or_cluster, resource_type, snapshot_at DESC);
CREATE INDEX IF NOT EXISTS idx_ics_saturation
    ON infrastructure_capacity_snapshots (saturation_ratio DESC NULLS LAST)
    WHERE saturation_ratio IS NOT NULL;

-- ── 2. Growth Forecast Models ─────────────────────────────────────────────────
-- Persists the fitted regression parameters and forward projections produced by
-- the forecasting pipeline (OLS / ARIMA-style).  One row per model run per metric.

CREATE TABLE IF NOT EXISTS growth_forecast_models (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Model identity
    model_name              TEXT        NOT NULL,           -- e.g. "ols_log_linear_v1"
    metric                  TEXT        NOT NULL            -- mirrors ForecastMetric enum
                                CHECK (metric IN ('tps','storage_gb','db_connections','memory_gb','cpu_cores','active_merchants','active_agents')),
    horizon                 TEXT        NOT NULL            -- mirrors ForecastHorizon enum
                                CHECK (horizon IN ('rolling_90d','annual_12m')),
    trained_on              DATE        NOT NULL,           -- date the model was fitted
    training_window_days    INT         NOT NULL DEFAULT 365,

    -- OLS regression weights (log-linear model: log(y) = beta0 + beta1·t)
    beta0                   DOUBLE PRECISION NOT NULL,      -- intercept
    beta1                   DOUBLE PRECISION NOT NULL,      -- slope (daily growth rate in log-space)
    residual_std_dev        DOUBLE PRECISION NOT NULL,      -- σ of residuals

    -- Prediction outcomes
    predicted_depletion_date DATE,                          -- NULL if no depletion projected
    days_until_depletion    INT,
    predicted_value_at_horizon DOUBLE PRECISION NOT NULL,   -- point estimate at end of horizon
    lower_bound_80          DOUBLE PRECISION NOT NULL,      -- 80% CI lower
    upper_bound_80          DOUBLE PRECISION NOT NULL,      -- 80% CI upper

    -- Accuracy tracking (filled in retrospectively)
    mape_pct                DOUBLE PRECISION,               -- mean absolute percentage error
    r_squared               DOUBLE PRECISION,               -- goodness of fit

    -- Thresholds used for depletion calculation
    capacity_threshold      DOUBLE PRECISION,               -- e.g. max storage GB provisioned
    warning_threshold_pct   NUMERIC(5,2) NOT NULL DEFAULT 80.00,  -- alert at 80% saturation
    critical_threshold_pct  NUMERIC(5,2) NOT NULL DEFAULT 95.00,

    -- Serialised full forecast series (date → predicted_value) for charting
    forecast_series         JSONB        NOT NULL DEFAULT '[]',

    -- Provenance
    model_version           TEXT        NOT NULL DEFAULT '1.0',
    computed_by             TEXT        NOT NULL DEFAULT 'capacity-forecaster',
    notes                   TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_gfm_metric_horizon
    ON growth_forecast_models (metric, horizon, trained_on DESC);
CREATE INDEX IF NOT EXISTS idx_gfm_depletion_date
    ON growth_forecast_models (predicted_depletion_date ASC NULLS LAST)
    WHERE predicted_depletion_date IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_gfm_trained_on
    ON growth_forecast_models (trained_on DESC);
