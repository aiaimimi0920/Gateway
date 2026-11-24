-- Only the live submit owner can attest that execute was never entered.
-- Legacy submitting rows remain unknown; do not backfill from timeouts or audits.
CREATE TABLE IF NOT EXISTS gateway_sap_response_unsent_proofs (
    response_id TEXT PRIMARY KEY REFERENCES gateway_sap_response_receipts(id),
    revision BIGINT NOT NULL CHECK (revision >= 0)
);
CREATE INDEX IF NOT EXISTS gateway_sap_response_recovery_open
    ON gateway_sap_response_balance_holds(response_id) WHERE open = 1;
