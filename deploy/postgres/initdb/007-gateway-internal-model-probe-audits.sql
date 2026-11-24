-- Additive Gateway-internal probe audit, independent of Platform caller and billing audits.
-- Existing databases require the deployment owner's reviewed migration and grants.
CREATE TABLE IF NOT EXISTS gateway_internal_model_probe_audits (
    id TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    product TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    credential_id TEXT NOT NULL,
    model TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'running'
        CHECK (status IN ('running', 'completed', 'failed', 'cancelled')),
    upstream_status INTEGER,
    duration_ms BIGINT CHECK (duration_ms >= 0),
    prompt_tokens BIGINT CHECK (prompt_tokens >= 0),
    completion_tokens BIGINT CHECK (completion_tokens >= 0),
    total_tokens BIGINT CHECK (total_tokens >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    completed_at TIMESTAMPTZ,
    CHECK ((status = 'running') = (completed_at IS NULL))
);
CREATE INDEX IF NOT EXISTS gateway_internal_model_probe_audits_scope_idx
    ON gateway_internal_model_probe_audits (namespace, product, provider_id, created_at DESC);
