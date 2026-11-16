-- Additive Gateway refresh evidence. Do not hand-edit the Platform-owned 001 dump.
-- Existing servers need schema-owner review, backup and transactional migration.
-- No runtime auto-DDL, expiration, credential cascade, or destructive rollback.
ALTER TABLE gateway_provider_accounts
    ADD COLUMN IF NOT EXISTS refresh_authority_id UUID NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN IF NOT EXISTS refresh_authority_version BIGINT NOT NULL DEFAULT 0;
ALTER TABLE gateway_provider_credentials
    ADD COLUMN IF NOT EXISTS refresh_authority_id UUID NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN IF NOT EXISTS refresh_authority_version BIGINT NOT NULL DEFAULT 0;

CREATE OR REPLACE FUNCTION gateway_refresh_authority_advance() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    NEW.refresh_authority_id := OLD.refresh_authority_id;
    NEW.refresh_authority_version := OLD.refresh_authority_version + 1;
    RETURN NEW;
END;
$$;

-- A normal object writer can replace bytes while retaining its object key.
-- Hence any payload UPDATE advances, even if the SQL values compare equal.
CREATE OR REPLACE TRIGGER gateway_account_refresh_payload_generation
BEFORE UPDATE OF payload_inline, payload_object_key, storage_mode ON gateway_provider_accounts
FOR EACH ROW EXECUTE FUNCTION gateway_refresh_authority_advance();
CREATE OR REPLACE TRIGGER gateway_credential_refresh_payload_generation
BEFORE UPDATE OF payload_inline, payload_object_key, storage_mode ON gateway_provider_credentials
FOR EACH ROW EXECUTE FUNCTION gateway_refresh_authority_advance();
CREATE OR REPLACE TRIGGER gateway_account_refresh_control_generation
BEFORE UPDATE OF id, adapter, status ON gateway_provider_accounts
FOR EACH ROW WHEN ((NEW.id, NEW.adapter, CASE WHEN NEW.status IN ('active','cooling') THEN 'enabled' ELSE NEW.status END)
    IS DISTINCT FROM (OLD.id, OLD.adapter, CASE WHEN OLD.status IN ('active','cooling') THEN 'enabled' ELSE OLD.status END))
EXECUTE FUNCTION gateway_refresh_authority_advance();
CREATE OR REPLACE TRIGGER gateway_credential_refresh_control_generation
BEFORE UPDATE OF id, provider_account_id, status, archived_at ON gateway_provider_credentials
FOR EACH ROW WHEN ((NEW.id, NEW.provider_account_id, CASE WHEN NEW.status IN ('active','cooling') THEN 'enabled' ELSE NEW.status END, NEW.archived_at)
    IS DISTINCT FROM (OLD.id, OLD.provider_account_id, CASE WHEN OLD.status IN ('active','cooling') THEN 'enabled' ELSE OLD.status END, OLD.archived_at))
EXECUTE FUNCTION gateway_refresh_authority_advance();

-- Records contain credential secrets, requiring the same access/backup protections.
-- Keep consumption evidence after account deletion: no FK or TTL may erase it.
CREATE TABLE IF NOT EXISTS gateway_provider_refresh_grants (
    family TEXT NOT NULL CHECK (family ~ '^[0-9a-f]{64}$'),
    material TEXT NOT NULL CHECK (material ~ '^[0-9a-f]{64}$'),
    record TEXT NOT NULL CHECK (octet_length(record) <= 524288),
    published_authority TEXT CHECK (published_authority ~ '^[0-9a-f]{64}$'),
    PRIMARY KEY (family, material)
);
CREATE TABLE IF NOT EXISTS gateway_provider_refresh_heads (
    family TEXT PRIMARY KEY CHECK (family ~ '^[0-9a-f]{64}$'),
    material TEXT NOT NULL CHECK (material ~ '^[0-9a-f]{64}$')
);
