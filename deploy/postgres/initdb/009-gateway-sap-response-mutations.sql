-- A committed claim permits one possible upstream mutation. It is never
-- reissued after a crash, timeout or an ambiguous transport result.
CREATE TABLE IF NOT EXISTS gateway_sap_response_mutations (
    response_id TEXT NOT NULL REFERENCES gateway_sap_response_receipts(id),
    operation TEXT NOT NULL CHECK (operation IN ('cancel', 'delete')),
    confirmed BIGINT NOT NULL DEFAULT 0 CHECK (confirmed IN (0, 1)),
    PRIMARY KEY (response_id, operation)
);
