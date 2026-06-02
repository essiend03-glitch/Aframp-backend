-- DeFi Analytics & Yield Performance Dashboard (Issue #348)
-- Snapshot tables for platform-wide, strategy, protocol, AMM, lending, and user analytics

-- ── Enums ─────────────────────────────────────────────────────────────────────

CREATE TYPE defi_report_period AS ENUM ('weekly', 'monthly', 'quarterly');
CREATE TYPE defi_analytics_report_status AS ENUM ('pending', 'ready', 'failed');

-- ── Platform DeFi Summary Snapshots ──────────────────────────────────────────

CREATE TABLE defi_platform_snapshots (
    snapshot_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    total_value_locked          NUMERIC(30, 8) NOT NULL DEFAULT 0,
    total_yield_distributed     NUMERIC(30, 8) NOT NULL DEFAULT 0,
    weighted_avg_yield_rate     DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_amm_liquidity         NUMERIC(30, 8) NOT NULL DEFAULT 0,
    total_collateral_locked     NUMERIC(30, 8) NOT NULL DEFAULT 0,
    total_outstanding_loans     NUMERIC(30, 8) NOT NULL DEFAULT 0,
    active_savings_positions    BIGINT NOT NULL DEFAULT 0,
    active_amm_positions        BIGINT NOT NULL DEFAULT 0,
    active_lending_positions    BIGINT NOT NULL DEFAULT 0,
    platform_defi_revenue       NUMERIC(30, 8) NOT NULL DEFAULT 0,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_defi_platform_snapshots_at ON defi_platform_snapshots(snapshot_at DESC);
CREATE INDEX idx_defi_platform_snapshots_period ON defi_platform_snapshots(period_start, period_end);

-- ── Strategy Performance Snapshots ───────────────────────────────────────────

CREATE TABLE defi_strategy_snapshots (
    snapshot_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy_id                 UUID NOT NULL,
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    total_allocated             NUMERIC(30, 8) NOT NULL DEFAULT 0,
    yield_earned                NUMERIC(30, 8) NOT NULL DEFAULT 0,
    effective_yield_rate        DOUBLE PRECISION NOT NULL DEFAULT 0,
    max_drawdown                DOUBLE PRECISION NOT NULL DEFAULT 0,
    risk_adjusted_return        DOUBLE PRECISION NOT NULL DEFAULT 0,
    rebalancing_event_count     INT NOT NULL DEFAULT 0,
    protocol_contributions      JSONB NOT NULL DEFAULT '{}',
    benchmark_yield_rate        DOUBLE PRECISION NOT NULL DEFAULT 0,
    benchmark_delta             DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_defi_strategy_snapshots_strategy ON defi_strategy_snapshots(strategy_id, period_start DESC);

-- ── Protocol Analytics Snapshots ─────────────────────────────────────────────

CREATE TABLE defi_protocol_snapshots (
    snapshot_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    protocol_id                 TEXT NOT NULL,
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    platform_exposure           NUMERIC(30, 8) NOT NULL DEFAULT 0,
    yield_earned                NUMERIC(30, 8) NOT NULL DEFAULT 0,
    fee_income                  NUMERIC(30, 8) NOT NULL DEFAULT 0,
    impermanent_loss            NUMERIC(30, 8) NOT NULL DEFAULT 0,
    health_score                DOUBLE PRECISION NOT NULL DEFAULT 0,
    uptime_pct                  DOUBLE PRECISION NOT NULL DEFAULT 100,
    capital_efficiency          DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_defi_protocol_snapshots_protocol ON defi_protocol_snapshots(protocol_id, period_start DESC);

-- ── AMM Pool Analytics Snapshots ─────────────────────────────────────────────

CREATE TABLE defi_amm_pool_snapshots (
    snapshot_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id                     TEXT NOT NULL,
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    trading_volume              NUMERIC(30, 8) NOT NULL DEFAULT 0,
    fee_income                  NUMERIC(30, 8) NOT NULL DEFAULT 0,
    impermanent_loss            NUMERIC(30, 8) NOT NULL DEFAULT 0,
    hold_strategy_return        NUMERIC(30, 8) NOT NULL DEFAULT 0,
    actual_yield                NUMERIC(30, 8) NOT NULL DEFAULT 0,
    capital_efficiency          DOUBLE PRECISION NOT NULL DEFAULT 0,
    price_range_coverage_pct    DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_defi_amm_pool_snapshots_pool ON defi_amm_pool_snapshots(pool_id, period_start DESC);

-- ── Lending Portfolio Snapshots ───────────────────────────────────────────────

CREATE TABLE defi_lending_snapshots (
    snapshot_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    total_collateral            NUMERIC(30, 8) NOT NULL DEFAULT 0,
    total_outstanding_loans     NUMERIC(30, 8) NOT NULL DEFAULT 0,
    avg_loan_to_value_ratio     DOUBLE PRECISION NOT NULL DEFAULT 0,
    avg_health_factor           DOUBLE PRECISION NOT NULL DEFAULT 0,
    liquidation_count           INT NOT NULL DEFAULT 0,
    liquidation_rate            DOUBLE PRECISION NOT NULL DEFAULT 0,
    interest_income             NUMERIC(30, 8) NOT NULL DEFAULT 0,
    unique_borrowers            INT NOT NULL DEFAULT 0,
    avg_loan_size               NUMERIC(30, 8) NOT NULL DEFAULT 0,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_defi_lending_snapshots_period ON defi_lending_snapshots(period_start DESC);

-- ── User DeFi Analytics Snapshots ────────────────────────────────────────────

CREATE TABLE defi_user_snapshots (
    snapshot_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id                   UUID NOT NULL,
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    total_deposited_savings     NUMERIC(30, 8) NOT NULL DEFAULT 0,
    total_yield_earned          NUMERIC(30, 8) NOT NULL DEFAULT 0,
    net_yield_rate              DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_collateral_locked     NUMERIC(30, 8) NOT NULL DEFAULT 0,
    outstanding_loan_balance    NUMERIC(30, 8) NOT NULL DEFAULT 0,
    net_defi_position_value     NUMERIC(30, 8) NOT NULL DEFAULT 0,
    product_usage               JSONB NOT NULL DEFAULT '{}',
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_defi_user_snapshots_wallet ON defi_user_snapshots(wallet_id, period_start DESC);

-- ── DeFi Analytics Reports ────────────────────────────────────────────────────

CREATE TABLE defi_analytics_reports (
    report_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    report_type                 defi_report_period NOT NULL,
    period_start                TIMESTAMPTZ NOT NULL,
    period_end                  TIMESTAMPTZ NOT NULL,
    status                      defi_analytics_report_status NOT NULL DEFAULT 'pending',
    report_data                 JSONB,
    download_url                TEXT,
    generated_at                TIMESTAMPTZ,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_defi_analytics_reports_type ON defi_analytics_reports(report_type, period_start DESC);
CREATE INDEX idx_defi_analytics_reports_status ON defi_analytics_reports(status);

-- ── Export Requests ───────────────────────────────────────────────────────────

CREATE TABLE defi_export_requests (
    export_id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requester_id                TEXT NOT NULL,
    export_scope                TEXT NOT NULL,  -- 'platform' or 'user'
    date_range_start            TIMESTAMPTZ NOT NULL,
    date_range_end              TIMESTAMPTZ NOT NULL,
    metric_set                  JSONB NOT NULL DEFAULT '[]',
    status                      TEXT NOT NULL DEFAULT 'pending',
    download_url                TEXT,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at                TIMESTAMPTZ
);

CREATE INDEX idx_defi_export_requests_requester ON defi_export_requests(requester_id, created_at DESC);
