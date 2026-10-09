# Local access keys

Local/desktop storage serves the access-key catalog and normal-key create, edit,
rotate, enable/disable, revoke and delete operations from `local/state/runtime.sqlite3`. The
management authorization boundary is unchanged. Issued tokens work with Bearer
and X-API-Key authentication, including `/v1/models` and inference requests.

Tokens contain cryptographically random material independent of the process's
environment API-key secret. Authentication persists SHA-256 token digests. A separate
AES-256-GCM sealed copy supports the management-only POST `access/keys/{id}/secret`
endpoint; its response is `no-store`. Catalog and update responses never reveal it.
The copy operation follows the console authentication and remote-access policy and
does not store plaintext in browser storage. The encryption key lives in
`local/state/access-key-seal.json`; protect and back it up with the private data directory.
This protects a database-only disclosure, not compromise of the entire data directory.
Hash-only keys from older versions remain valid and become copyable after a verified
client use, without rotation. Until then, copying reports that no secret has been saved;
the operator may explicitly rotate instead. Missing/corrupt seal material fails closed
for copy/issuance, never silently re-keys existing secrets or disables hash authentication.
Restarting or upgrading preserves issued keys. Rotation atomically revokes the
old key and issues a replacement retaining identity, expiry and restrictions.
Revocation prevents subsequent authorization; already dispatched requests may finish.

The key list offers an Enable/Disable action and a separate confirmed Delete action.
Management-only POST `access/keys/{id}/enabled` accepts `{"enabled":true|false}`.
It changes only `active`/`disabled` state and the update timestamp; the token, expiry,
quota, group bindings and cash history remain unchanged. Re-enabling does not extend
an expired key. Revoked keys (including old rotated tokens) cannot be re-enabled.
DELETE `access/keys/{id}` removes the credential from the list and prevents future
authorization; this cannot be undone. Historical cash receipts and in-flight cash
settlement remain owned by the cash account, not deleted with the credential.
Deleting the row removes its list-level bill entry; retained bills remain queryable
by the management cash-ledger API using the original key ID.

Local normal keys authorize the operator's local route document within their selected
entitlement groups. Keys issued before group binding retain their previous restrictions.
New and upgraded
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

The API requires nonempty owner/project/tenant IDs and a display name. The key
manager uses the catalog's additive `storageMode` field to select the creation
form. Confirmed local storage pre-fills `local` attribution namespaces, allowing
creation with a name, at least one entitlement group and an optional validity period. Advanced fields remain
editable. Server mode and older catalogs without this field still require explicit
owner/project/tenant IDs; the browser origin is never used to infer local rights.
The page generates tokens on the server, with a copyable API Base URL and a persistent
Copy Key action. Create and Edit share a secondary dialog for name, validity, groups
and quota. Rotation, state changes and deletion require confirmation. Local
project/tenant IDs are attribution namespaces, not provisioned Platform tenants.
Project/tenant and token prefix cannot change during edit; issue another key to
change identity. `expiresAt` must be RFC3339. Revoked or expired keys cannot be
reactivated through edit or rotation.

The optional create/update `quota` object accepts `unlimited` (no limit),
`message_prepaid` or `token_prepaid` with an integer total limit from 0 to
9,007,199,254,740,991. Key metadata and quota save in one transaction; failed quota
updates cannot partially rename a key or change its groups. Same-unit edits preserve
consumed and reserved credit, including a negative remaining balance after lowering
the total below past usage. Omitting quota preserves all existing balance settings.
Switching away from and back to a unit does not reset its prior consumption. Unlimited
usage is not retroactively charged. Existing status/period restrictions remain in force.
Local token/request reservations reject unit changes while requests are in flight. PostgreSQL has no durable
token/request in-flight counter, so this editor rejects changing either existing prepaid unit; same-unit
total changes and initial limits remain supported. Use a new key for another unit on
the server. Server deployment/integration acceptance is separate from local EXE tests.

Cash quotas additionally accept `cash_prepaid` with an integer micro-USD `limit` and
`currency: "USD"`. The catalog's `cashQuotaSupported` capability guards old servers.
Cash settlement has a separate durable account and immutable per-request tariff,
not a token conversion or display-cost estimate. See [Key cash billing](key-cash-billing.md)
for pricing, concurrent reservations, supported requests, unresolved usage and upgrades.

Optional metadata permits only these string arrays:

- `scope` or `scopes`: `relay` for all inference endpoints, or endpoint names such
  as `chat.completions`, `responses`, `models`. An explicit empty list denies all.
- `models`: exact requested model names. Empty denies every model.
- `providerIds`: local provider account IDs. Empty denies every provider.
- `accountGroupIds`: up to 128 exact route entitlement-group IDs. Multiple groups
  grant the union of their currently enabled members, intersected with other key
  restrictions and any trusted request group. Empty denies all accounts. Missing,
  removed or disabled groups never fall back to unrestricted routing.

The key manager lists the groups and supports changing bindings without rotating
the secret. Editing preserves other metadata restrictions and server bundle bindings.
Create/update validates selected groups; dispatch resolves membership from one current
route snapshot before account selection, affinity and fallback. Model discovery is
restricted to models served by permitted accounts. Group edits affect subsequent
requests; already dispatched work may complete. Rotation and restart preserve bindings.
The catalog's optional `accountGroups` field advertises binding support; older gateways
cannot create group-bound keys through this UI. Unbound legacy keys are explicitly
labelled and can be narrowed by editing their groups.

Server-mode group restrictions narrow the existing PostgreSQL bundle entitlements;
group membership does not create Platform rights or bypass project/tenant boundaries.

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
