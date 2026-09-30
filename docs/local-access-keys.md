# Local access keys

Local/desktop storage serves the access-key catalog and normal-key create, edit,
rotate, revoke and delete operations from `local/state/runtime.sqlite3`. The
management authorization boundary is unchanged. Issued tokens work with Bearer
and X-API-Key authentication, including `/v1/models` and inference requests.

Tokens contain cryptographically random material independent of the process's
environment API-key secret. Only SHA-256 token digests are persisted. Create and
rotate return the token once; catalog and update responses never reveal it.
Restarting or upgrading preserves issued keys. Rotation atomically revokes the
old key and issues a replacement retaining identity, expiry and restrictions.
Revocation prevents subsequent authorization; already dispatched requests may finish.

Local normal keys authorize the operator's local route document. New and upgraded
local keys receive an explicit `unlimited` balance, preserving previously issued
keys' behavior. Every inference request evaluates the shared balance policy,
including requests without text. The management balance endpoints can change a key
to `time_pass`, `message_prepaid` (also accepting `request_prepaid`), or `token_prepaid`.
Inactive, expired, out-of-period and exhausted balances deny requests before upstream
execution. Server catalog bundles and aggregate keys retain their server owners;
local creation rejects these unsupported rights rather than granting them implicitly.
The panel's default `user` kind becomes `normal`.

`AccessBalanceStore` owns the common get/adjust/evaluate/reserve/refund/settle interface.
SQLite uses `BEGIN IMMEDIATE`; PostgreSQL locks the key and balance rows in one
transaction. PostgreSQL remains the server ledger even when Redis contains an older
balance projection. Checked arithmetic rejects overflow and invalid timestamps.
Refunds and usage settlement change remaining credit without increasing purchased
totals. Missing upstream usage retains the reservation rather than manufacturing
usage or leaving finalization unfinished.

Local key IDs bind to stable balance accounts. Rotation shares the account, so an
old in-flight request settles against the replacement's balance. Deleted key bindings
remain private only while requests are in flight, then maintenance removes them.
Changing balance mode while a local request is in flight is rejected. Instance
heartbeats retire abandoned holds after a crash; their debit is retained because
actual upstream consumption is unknown. The account and key limits are 10,000.

The operator supplies nonempty owner/project/tenant IDs and a display name. Local
project/tenant IDs are attribution namespaces, not provisioned Platform tenants.
Project/tenant and token prefix cannot change during edit; issue another key to
change identity. `expiresAt` must be RFC3339. Revoked or expired keys cannot be
reactivated through edit or rotation.

Optional metadata permits only these string arrays:

- `scope` or `scopes`: `relay` for all inference endpoints, or endpoint names such
  as `chat.completions`, `responses`, `models`. An explicit empty list denies all.
- `models`: exact requested model names. Empty denies every model.
- `providerIds`: local provider account IDs. Empty denies every provider.

Omitted restrictions allow the local routes. Candidate filtering applies before
selection and fallback, and model discovery respects model/provider restrictions.
Explicit credential-reference overrides are denied for local issued keys.
Unknown or malformed metadata is rejected. Local mode does not fall through to
unauthenticated development access when a key is missing or invalid.

Startup installs additive key, balance-account and in-flight ownership tables transactionally for both new and
previously released schema-1 databases. Neither `PRAGMA user_version` nor the root
storage version changes. Existing credentials, refill tasks and audit history are
not rewritten. No manual migration command is required.

Current upstream development and real-call acceptance are restricted to NVIDIA.
Other credential pools are not implemented, modified or called by this task.
