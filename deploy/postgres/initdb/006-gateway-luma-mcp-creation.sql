-- Additive Server-only Luma MCP management journal, separate from public request audits.
-- Apply to existing databases only via the owning deployment's reviewed migration.
CREATE TABLE IF NOT EXISTS gateway_luma_mcp_creation_tasks (
    id TEXT PRIMARY KEY,
    owner_tag BYTEA NOT NULL CHECK (octet_length(owner_tag) = 32),
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    lease_until BIGINT NOT NULL DEFAULT 0 CHECK (lease_until >= 0),
    payload JSONB NOT NULL CHECK (jsonb_typeof(payload) = 'object'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
