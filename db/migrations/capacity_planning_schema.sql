-- ============================================================================
-- CAPACITY PLANNING & FORECASTING ENGINE — Database Schema
-- Issue #CAPACITY-001
--
-- Tables:
--   1. capacity_business_metrics      — daily business driver snapshots
--   2. capacity_resource_units        — Resource Consumption Unit (RCU) model
--   3. capacity_forecasts             — time-series forecast outputs
--   4. capacity_scenarios             — what-if simulation runs
--   5. capacity_cost_projections      — cloud cost projections per scenario
--   6. capacity_alerts                — early-warning threshold breaches
--   7. capacity_quarterly_reports     — quarterly report records
-- ============================================================================

-- ============================================================================
-- 1. CAPACITY_BUSINESS_METRICS
--    Daily snapshot of business drivers ingested from the platform.
--    Source of truth for the forecasting engine.
-- ============================================================================
CREATE TABLE capacity_business_metrics (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    metric_date             DATE        NOT NULL UNIQUE,
    -- Business drivers
    active_merchants        INTEGER     NOT NULL DEFAULT 0,
    active_agents           INTEGER     NOT NULL DEFAULT 0,
    daily_transactions      BIGINT      NOT NULL DEFAULT 0,
    -- Derived technical metrics (computed at ingest time)
    peak_tps                NUMERIC(10,4) NOT NULL DEFAULT 0,
    avg_transaction_size_kb NUMERIC(10,4) NOT NULL DEFAULT 0,
    api_call_volume         BIGINT      NOT NULL DEFAULT 0,
    db_connections_peak     INTEGER     NOT NULL DEFAULT 0,
    -- Storage
    storage_used_gb         NUMERIC(12,4) NOT NULL DEFAULT 0,
    storage_growth_gb       NUMERIC(12,4) NOT NULL DEFAULT 0,
    -- Compute
    avg_cpu_pct             NUMERIC(6,2) NOT NULL DEFAULT 0,
    avg_memory_gb           NUMERIC(10,4) NOT NULL DEFAULT 0,
    -- Corridor breakdown (JSON: { "ghana": 1200, "kenya": 800 })
    corridor_breakdown      JSONB       NOT NULL DEFAULT '{}',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_cbm_date ON capacity_business_metrics(metric_date DESC);

-- ============================================================================
-- 2. CAPACITY_RESOURCE_UNITS
--    Resource Consumption Unit (RCU) model — maps business drivers to
--    technical resource requirements. Updated monthly from actuals.
-- ============================================================================
CREATE TABLE capacity_resource_units (
    id                          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Month this model is valid for
    model_month                 DATE        NOT NULL UNIQUE,
    -- Per-1000-transactions resource cost
    cpu_cores_per_1k_tps        NUMERIC(10,6) NOT NULL,
    memory_gb_per_1k_tps        NUMERIC(10,6) NOT NULL,
    disk_iops_per_1k_tps        NUMERIC(10,4) NOT NULL,
    storage_gb_per_1k_tx        NUMERIC(10,6) NOT NULL,
    -- Per-active-agent
    db_connections_per_agent    NUMERIC(10,4) NOT NULL,
    -- Per-active-merchant
    db_connections_per_merchant NUMERIC(10,4) NOT NULL,
    -- Per-API-call
    memory_mb_per_api_call      NUMERIC(10,6) NOT NULL,
    -- Overhead multiplier (burst headroom, e.g. 1.3 = 30% headroom)
    overhead_multiplier         NUMERIC(6,4) NOT NULL DEFAULT 1.30,
    -- Forecast accuracy from previous month (MAE %)
    forecast_accuracy_pct       NUMERIC(6,2),
    -- Who computed this model
    computed_by                 VARCHAR(50) NOT NULL DEFAULT 'capacity_worker',
    notes                       TEXT,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_rcu_month ON capacity_resource_units(model_month DESC);

-- ============================================================================
-- 3. CAPACITY_FORECASTS
--    Output of the time-series forecasting engine (ARIMA-style linear
--    regression with trend + seasonality). One row per metric per horizon.
-- ============================================================================
CREATE TYPE forecast_horizon AS ENUM ('rolling_90d', 'annual_12m');
CREATE TYPE forecast_metric  AS ENUM (
    'tps', 'storage_gb', 'db_connections', 'memory_gb', 'cpu_cores', 'active_merchants', 'active_agents'
);

CREATE TABLE capacity_forecasts (
    id                  UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    forecast_date       DATE            NOT NULL,   -- date this forecast was generated
    target_date         DATE            NOT NULL,   -- date being forecast
    horizon             forecast_horizon NOT NULL,
    metric              forecast_metric  NOT NULL,
    predicted_value     NUMERIC(18,4)   NOT NULL,
    lower_bound         NUMERIC(18,4)   NOT NULL,   -- 80% confidence interval
    upper_bound         NUMERIC(18,4)   NOT NULL,
    -- Actual value (backfilled when the target_date passes)
    actual_value        NUMERIC(18,4),
    -- Absolute percentage error (backfilled)
    ape_pct             NUMERIC(8,4),
    model_version       VARCHAR(20)     NOT NULL DEFAULT 'arima_v1',
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT CURRENT_TIMESTAMP,

    UNIQUE (forecast_date, target_date, horizon, metric)
);

CREATE INDEX idx_cf_target_date ON capacity_forecasts(target_date DESC, metric);
CREATE INDEX idx_cf_horizon     ON capacity_forecasts(horizon, forecast_date DESC);

-- ============================================================================
-- 4. CAPACITY_SCENARIOS
--    What-if simulation runs. Each row is one simulation with its inputs
--    and computed resource impact.
-- ============================================================================
CREATE TABLE capacity_scenarios (
    id                          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name                        VARCHAR(200) NOT NULL,
    description                 TEXT,
    -- Simulation inputs
    transaction_volume_multiplier NUMERIC(8,4) NOT NULL DEFAULT 1.0,
    timeframe_months            INTEGER     NOT NULL DEFAULT 3,
    new_merchant_chains         INTEGER     NOT NULL DEFAULT 0,
    new_agent_count             INTEGER     NOT NULL DEFAULT 0,
    -- Projected resource outputs
    projected_peak_tps          NUMERIC(10,4),
    projected_storage_gb        NUMERIC(12,4),
    projected_memory_gb         NUMERIC(10,4),
    projected_cpu_cores         NUMERIC(10,4),
    projected_db_connections    INTEGER,
    -- Cost impact
    projected_monthly_cost_usd  NUMERIC(14,2),
    cost_delta_vs_baseline_usd  NUMERIC(14,2),
    -- Cloud provider used for cost calc
    cloud_provider              VARCHAR(20) NOT NULL DEFAULT 'aws',
    -- Full breakdown (JSON)
    resource_breakdown          JSONB       NOT NULL DEFAULT '{}',
    cost_breakdown              JSONB       NOT NULL DEFAULT '{}',
    -- Who ran this
    created_by                  VARCHAR(100) NOT NULL DEFAULT 'system',
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_cs_created ON capacity_scenarios(created_at DESC);

-- ============================================================================
-- 5. CAPACITY_COST_PROJECTIONS
--    Monthly cloud cost projections per resource type.
--    Supports multi-cloud (aws / gcp / azure).
-- ============================================================================
CREATE TABLE capacity_cost_projections (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    projection_month    DATE        NOT NULL,
    cloud_provider      VARCHAR(20) NOT NULL DEFAULT 'aws',
    -- Resource quantities
    cpu_cores           NUMERIC(10,4) NOT NULL,
    memory_gb           NUMERIC(10,4) NOT NULL,
    storage_gb          NUMERIC(12,4) NOT NULL,
    db_connections      INTEGER     NOT NULL,
    -- Unit costs (USD/month)
    cpu_cost_usd        NUMERIC(14,4) NOT NULL,
    memory_cost_usd     NUMERIC(14,4) NOT NULL,
    storage_cost_usd    NUMERIC(14,4) NOT NULL,
    db_cost_usd         NUMERIC(14,4) NOT NULL,
    -- Total
    total_cost_usd      NUMERIC(14,2) NOT NULL,
    -- MoM delta
    prev_month_cost_usd NUMERIC(14,2),
    cost_delta_pct      NUMERIC(8,2),
    -- Source: 'forecast' | 'actual' | 'scenario'
    source              VARCHAR(20) NOT NULL DEFAULT 'forecast',
    scenario_id         UUID REFERENCES capacity_scenarios(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    UNIQUE (projection_month, cloud_provider, source, COALESCE(scenario_id, '00000000-0000-0000-0000-000000000000'::uuid))
);

CREATE INDEX idx_ccp_month ON capacity_cost_projections(projection_month DESC, cloud_provider);

-- ============================================================================
-- 6. CAPACITY_ALERTS
--    Early-warning alerts fired when a resource is projected to breach
--    a threshold within the configured lead time (default: 60 days).
-- ============================================================================
CREATE TYPE capacity_alert_severity AS ENUM ('warning', 'critical');
CREATE TYPE capacity_alert_resource AS ENUM (
    'storage', 'tps', 'memory', 'cpu', 'db_connections', 'cost'
);

CREATE TABLE capacity_alerts (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    resource                capacity_alert_resource NOT NULL,
    severity                capacity_alert_severity NOT NULL,
    -- When the breach is projected to occur
    projected_breach_date   DATE        NOT NULL,
    -- Days until breach at time of alert
    days_until_breach       INTEGER     NOT NULL,
    current_value           NUMERIC(18,4) NOT NULL,
    threshold_value         NUMERIC(18,4) NOT NULL,
    projected_value         NUMERIC(18,4) NOT NULL,
    message                 TEXT        NOT NULL,
    -- Notification state
    notified_at             TIMESTAMPTZ,
    acknowledged_by         VARCHAR(100),
    acknowledged_at         TIMESTAMPTZ,
    resolved_at             TIMESTAMPTZ,
    -- Review task created for infra team
    review_task_id          VARCHAR(100),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_ca_unresolved ON capacity_alerts(created_at DESC) WHERE resolved_at IS NULL;
CREATE INDEX idx_ca_resource   ON capacity_alerts(resource, projected_breach_date ASC);

-- ============================================================================
-- 7. CAPACITY_QUARTERLY_REPORTS
--    Quarterly capacity report records.
-- ============================================================================
CREATE TABLE capacity_quarterly_reports (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    quarter                 VARCHAR(7)  NOT NULL UNIQUE, -- e.g. "2026-Q2"
    report_date             DATE        NOT NULL,
    -- Growth summary (JSON)
    growth_summary          JSONB       NOT NULL DEFAULT '{}',
    -- Capacity requirements per scenario (JSON)
    capacity_requirements   JSONB       NOT NULL DEFAULT '{}',
    -- Provisioning recommendations (JSON array)
    recommendations         JSONB       NOT NULL DEFAULT '[]',
    -- Forecast accuracy from previous quarter
    prev_quarter_accuracy_pct NUMERIC(6,2),
    -- Management-facing plain-language summary
    executive_summary       TEXT,
    -- Full report (JSON)
    full_report             JSONB       NOT NULL DEFAULT '{}',
    generated_by            VARCHAR(50) NOT NULL DEFAULT 'capacity_worker',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_cqr_quarter ON capacity_quarterly_reports(quarter DESC);
