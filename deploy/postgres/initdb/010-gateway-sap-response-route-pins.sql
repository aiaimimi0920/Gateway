-- Immutable, non-secret route identity. Old receipts remain readable for
-- accounting, but cannot be routed without an original pin.
CREATE TABLE IF NOT EXISTS gateway_sap_response_route_pins (
    response_id TEXT PRIMARY KEY REFERENCES gateway_sap_response_receipts(id),
    public_model TEXT NOT NULL CHECK (length(public_model) BETWEEN 1 AND 256),
    source_key_ref TEXT NOT NULL CHECK (length(source_key_ref) BETWEEN 1 AND 256),
    platform_access_ref TEXT CHECK (length(platform_access_ref) BETWEEN 1 AND 256)
);
