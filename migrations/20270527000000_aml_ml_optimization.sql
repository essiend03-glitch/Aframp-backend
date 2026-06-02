-- Migration: AML ML Optimization Layer — Issue #394
-- Tables for model versioning, training samples, shadow evaluations, and drift metrics

-- ---------------------------------------------------------------------------
-- Model versions (champion/challenger registry)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS aml_model_versions (
    model_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version              INTEGER NOT NULL,
    weights_json         JSONB NOT NULL,          -- [f64; 10] weight array
    bias                 DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    trained_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    training_samples     BIGINT NOT NULL DEFAULT 0,
    validation_precision DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    validation_recall    DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    fp_rate              DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    is_champion          BOOLEAN NOT NULL DEFAULT false,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Only one champion at a time
CREATE UNIQUE INDEX IF NOT EXISTS idx_aml_model_versions_champion
    ON aml_model_versions (is_champion)
    WHERE is_champion = true;

CREATE INDEX IF NOT EXISTS idx_aml_model_versions_version
    ON aml_model_versions (version DESC);

-- Seed the default model (prior weights) so the system starts with a champion
INSERT INTO aml_model_versions
    (model_id, version, weights_json, bias, training_samples,
     validation_precision, validation_recall, fp_rate, is_champion)
VALUES (
    gen_random_uuid(),
    0,
    '[-0.8, -0.6, -0.7, -0.3, 0.5, 0.6, 0.4, 0.9, 0.5, -0.9]'::jsonb,
    0.0,
    0,
    0.0,
    0.0,
    0.0,
    true
)
ON CONFLICT DO NOTHING;

-- ---------------------------------------------------------------------------
-- Training samples — labeled analyst decisions
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS aml_training_samples (
    sample_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_id                UUID NOT NULL,
    -- Feature columns (normalised 0.0–1.0)
    velocity_24h            DOUBLE PRECISION NOT NULL,
    velocity_7d             DOUBLE PRECISION NOT NULL,
    amount_ratio_30d        DOUBLE PRECISION NOT NULL,
    counterparty_diversity  DOUBLE PRECISION NOT NULL,
    known_counterparty_ratio DOUBLE PRECISION NOT NULL,
    kyc_tier_score          DOUBLE PRECISION NOT NULL,
    account_age_score       DOUBLE PRECISION NOT NULL,
    historical_fp_rate      DOUBLE PRECISION NOT NULL,
    geo_consistency         DOUBLE PRECISION NOT NULL,
    corridor_risk           DOUBLE PRECISION NOT NULL,
    -- Label
    is_false_positive       BOOLEAN NOT NULL,
    analyst_id              UUID NOT NULL,
    resolved_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_aml_training_samples_resolved_at
    ON aml_training_samples (resolved_at ASC);

CREATE INDEX IF NOT EXISTS idx_aml_training_samples_alert_id
    ON aml_training_samples (alert_id);

-- ---------------------------------------------------------------------------
-- Shadow evaluations — champion vs challenger comparison
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS aml_shadow_evaluations (
    eval_id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_id                    UUID NOT NULL,
    champion_model_id           UUID NOT NULL REFERENCES aml_model_versions(model_id),
    challenger_model_id         UUID NOT NULL REFERENCES aml_model_versions(model_id),
    champion_fp_probability     DOUBLE PRECISION NOT NULL,
    challenger_fp_probability   DOUBLE PRECISION NOT NULL,
    champion_recommendation     TEXT NOT NULL CHECK (champion_recommendation IN ('Suppress', 'Downgrade', 'Retain')),
    challenger_recommendation   TEXT NOT NULL CHECK (challenger_recommendation IN ('Suppress', 'Downgrade', 'Retain')),
    -- Filled in when analyst resolves the alert
    analyst_confirmed_fp        BOOLEAN,
    evaluated_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (alert_id, challenger_model_id)
);

CREATE INDEX IF NOT EXISTS idx_aml_shadow_evals_challenger
    ON aml_shadow_evaluations (challenger_model_id, evaluated_at DESC);

CREATE INDEX IF NOT EXISTS idx_aml_shadow_evals_champion
    ON aml_shadow_evaluations (champion_model_id, evaluated_at DESC);

CREATE INDEX IF NOT EXISTS idx_aml_shadow_evals_feedback
    ON aml_shadow_evaluations (analyst_confirmed_fp)
    WHERE analyst_confirmed_fp IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Drift metrics — periodic PSI and accuracy checks
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS aml_drift_metrics (
    metric_id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_id                UUID NOT NULL REFERENCES aml_model_versions(model_id),
    checked_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    max_psi                 DOUBLE PRECISION NOT NULL,
    critical_features_json  JSONB NOT NULL DEFAULT '[]'::JSONB,
    current_precision       DOUBLE PRECISION NOT NULL,
    current_recall          DOUBLE PRECISION NOT NULL,
    precision_drop          DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    recall_drop             DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    alert_triggered         BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_aml_drift_metrics_model
    ON aml_drift_metrics (model_id, checked_at DESC);

CREATE INDEX IF NOT EXISTS idx_aml_drift_metrics_alerts
    ON aml_drift_metrics (alert_triggered, checked_at DESC)
    WHERE alert_triggered = true;

-- ---------------------------------------------------------------------------
-- ML scoring audit log — every suppression/downgrade must be auditable
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS aml_ml_scoring_audit (
    audit_id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_id            UUID NOT NULL,
    model_id            UUID NOT NULL REFERENCES aml_model_versions(model_id),
    model_version       INTEGER NOT NULL,
    fp_probability      DOUBLE PRECISION NOT NULL,
    recommendation      TEXT NOT NULL CHECK (recommendation IN ('Suppress', 'Downgrade', 'Retain')),
    attributions_json   JSONB NOT NULL,   -- SHAP feature attributions
    justification       TEXT NOT NULL,    -- human-readable for compliance
    scored_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_aml_ml_audit_alert
    ON aml_ml_scoring_audit (alert_id, scored_at DESC);

CREATE INDEX IF NOT EXISTS idx_aml_ml_audit_recommendation
    ON aml_ml_scoring_audit (recommendation, scored_at DESC);

COMMENT ON TABLE aml_model_versions     IS 'AML ML model registry — champion/challenger versioning';
COMMENT ON TABLE aml_training_samples   IS 'Analyst-labeled TP/FP samples for supervised training';
COMMENT ON TABLE aml_shadow_evaluations IS 'Champion vs challenger shadow mode comparison records';
COMMENT ON TABLE aml_drift_metrics      IS 'PSI-based feature drift and accuracy degradation checks';
COMMENT ON TABLE aml_ml_scoring_audit   IS 'Immutable audit log of every ML suppression/downgrade decision';
