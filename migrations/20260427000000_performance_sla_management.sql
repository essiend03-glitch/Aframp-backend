-- Performance SLA Management & Breach Response (Issue #405)
-- Tracks SLO definitions, real-time compliance windows, breach incidents,
-- post-mortems, and monthly partner compliance reports.

-- ── SLO definitions ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS slo_definitions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL UNIQUE,          -- e.g. "api_p99_latency"
    description     TEXT,
    metric_name     TEXT NOT NULL,                 -- Prometheus metric to evaluate
    operator        TEXT NOT NULL CHECK (operator IN ('lt','lte','gt','gte')),
    threshold       NUMERIC(18,6) NOT NULL,        -- e.g. 0.5 (seconds) or 99.99 (%)
    window_seconds  INT  NOT NULL DEFAULT 300,     -- evaluation window (5 min default)
    severity        TEXT NOT NULL DEFAULT 'high' CHECK (severity IN ('critical','high','medium','low')),
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── SLA breach incidents ──────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS sla_breach_incidents (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slo_id              UUID NOT NULL REFERENCES slo_definitions(id),
    detected_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at         TIMESTAMPTZ,
    status              TEXT NOT NULL DEFAULT 'open'
                            CHECK (status IN ('open','investigating','resolved','post_mortem_pending','closed')),
    observed_value      NUMERIC(18,6) NOT NULL,
    threshold_value     NUMERIC(18,6) NOT NULL,
    affected_service    TEXT NOT NULL,
    root_cause_summary  TEXT,
    remediation_steps   TEXT,
    -- forensic context attached automatically at breach time
    context_snapshot    JSONB NOT NULL DEFAULT '{}',
    -- communication tracking
    partners_notified   BOOLEAN NOT NULL DEFAULT FALSE,
    notification_sent_at TIMESTAMPTZ,
    etr                 TIMESTAMPTZ,               -- estimated time to resolution
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── Post-mortem records ───────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS sla_post_mortems (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    incident_id     UUID NOT NULL UNIQUE REFERENCES sla_breach_incidents(id),
    author          TEXT NOT NULL,
    timeline        JSONB NOT NULL DEFAULT '[]',   -- [{at, event}]
    root_cause      TEXT NOT NULL,
    contributing_factors TEXT,
    remediation     TEXT NOT NULL,
    preventive_measures TEXT NOT NULL,
    action_items    JSONB NOT NULL DEFAULT '[]',   -- [{owner, task, due_date, done}]
    status          TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','review','approved')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── Monthly SLA compliance reports ───────────────────────────────────────────
CREATE TABLE IF NOT EXISTS sla_compliance_reports (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    partner_id      UUID,                          -- NULL = platform-wide report
    report_month    DATE NOT NULL,                 -- first day of the month
    total_breaches  INT  NOT NULL DEFAULT 0,
    mttr_seconds    NUMERIC(12,2),                 -- mean time to resolve
    availability_pct NUMERIC(7,4),                 -- e.g. 99.9812
    breach_ids      UUID[] NOT NULL DEFAULT '{}',
    generated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (partner_id, report_month)
);

-- ── Indexes ───────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_sla_breaches_status     ON sla_breach_incidents(status);
CREATE INDEX IF NOT EXISTS idx_sla_breaches_detected   ON sla_breach_incidents(detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_sla_breaches_slo        ON sla_breach_incidents(slo_id);
CREATE INDEX IF NOT EXISTS idx_sla_reports_month       ON sla_compliance_reports(report_month DESC);

-- ── Seed default SLO definitions ─────────────────────────────────────────────
INSERT INTO slo_definitions (name, description, metric_name, operator, threshold, window_seconds, severity)
VALUES
  ('api_p99_latency',    'API P99 latency must stay below 500ms',
   'aframp_http_request_duration_seconds', 'lt', 0.5,   300, 'critical'),
  ('api_availability',   'API availability must stay above 99.9%',
   'aframp_http_requests_total',           'gte', 99.9,  300, 'critical'),
  ('onramp_success_rate','Onramp success rate must stay above 95%',
   'aframp_onramp_success_rate',           'gte', 95.0,  600, 'high'),
  ('offramp_success_rate','Offramp success rate must stay above 95%',
   'aframp_offramp_success_rate',          'gte', 95.0,  600, 'high'),
  ('db_query_p95',       'DB query P95 must stay below 200ms',
   'aframp_db_query_duration_seconds',     'lt', 0.2,   300, 'high')
ON CONFLICT (name) DO NOTHING;
