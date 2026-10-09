-- Additive cash ledger. Existing servers require a separate deployment review before applying this expansion.
-- The local SQLite startup runs the same schema inside its existing startup transaction.
CREATE TABLE IF NOT EXISTS gateway_cash_accounts (
    id TEXT PRIMARY KEY,
    payload TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS gateway_cash_keys (
    key_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES gateway_cash_accounts(id)
);
CREATE INDEX IF NOT EXISTS gateway_cash_keys_account ON gateway_cash_keys(account_id);
CREATE TABLE IF NOT EXISTS gateway_cash_requests (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES gateway_cash_accounts(id),
    payload TEXT NOT NULL,
    created TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS gateway_cash_requests_account ON gateway_cash_requests(account_id, created DESC);
