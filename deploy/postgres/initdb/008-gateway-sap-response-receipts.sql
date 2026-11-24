-- Additive SAP Responses journal; no request/response payload or Service Key is stored.
-- SQLite startup and standalone PostgreSQL initdb use this same schema.
CREATE TABLE IF NOT EXISTS gateway_sap_response_receipts (
    id TEXT PRIMARY KEY,
    owner_tag TEXT NOT NULL CHECK (length(owner_tag) = 64),
    binding_tag TEXT NOT NULL CHECK (length(binding_tag) = 64),
    provider_ref TEXT NOT NULL CHECK (length(provider_ref) BETWEEN 1 AND 256),
    credential_ref TEXT NOT NULL CHECK (length(credential_ref) BETWEEN 1 AND 256),
    upstream_id TEXT,
    phase TEXT NOT NULL CHECK (phase IN ('submitting', 'pending', 'completed', 'failed', 'deleted')),
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    input_tokens BIGINT CHECK (input_tokens >= 0),
    output_tokens BIGINT CHECK (output_tokens >= 0),
    settled BIGINT NOT NULL DEFAULT 0 CHECK (settled IN (0, 1)),
    CHECK ((phase = 'submitting') = (upstream_id IS NULL)),
    CHECK ((input_tokens IS NULL) = (output_tokens IS NULL)),
    CHECK (phase NOT IN ('submitting', 'pending') OR (input_tokens IS NULL AND settled = 0)),
    CHECK (phase <> 'deleted' OR settled = 1),
    UNIQUE (binding_tag, upstream_id)
);

-- A hold survives process cleanup; only the original receipt can close it.
CREATE TABLE IF NOT EXISTS gateway_sap_response_balance_holds (
    response_id TEXT PRIMARY KEY REFERENCES gateway_sap_response_receipts(id),
    key_ref TEXT NOT NULL,
    account_ref TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('token_prepaid', 'message_prepaid', 'request_prepaid', 'unlimited', 'time_pass')),
    reserved BIGINT NOT NULL CHECK (reserved >= 0),
    open BIGINT NOT NULL DEFAULT 1 CHECK (open IN (0, 1))
);
CREATE INDEX IF NOT EXISTS gateway_sap_response_open_holds
    ON gateway_sap_response_balance_holds(account_ref) WHERE open = 1;
