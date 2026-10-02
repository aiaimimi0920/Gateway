# Provider credential pool capacity

The provider policy is additive and keeps existing route documents readable:

- `pool_min_size`: automatic refill starts when verified available credentials are
  strictly below this value. Default 1; zero disables the automatic trigger.
- `pool_target_size`: retained legacy field name for the maximum refill target.
  Default 100. Values are positive JSON-safe integers, not capped at 999999 or
  10000. The minimum must not exceed the maximum.
- `pool_refill_in_progress`: backend-owned, optional durable hysteresis state.
  Partial automatic delivery keeps this flag set across restarts. Refill continues
  after passing the minimum until verified availability reaches the maximum.
  Disabling automatic refill or setting minimum zero clears the flag.

Available means the console's green health state: an enabled credential with at
least one active model and no credential-wide failure. Model-specific failures do
not invalidate another working model. Unknown, degraded/cooling and blocked
credentials are not reported as available. SQLite and PostgreSQL use the same
aggregation, without the management endpoint's 1000-row truncation.

New unobserved credentials reserve capacity while verification is pending.
Outstanding queue tasks reserve their requested amount against direct automation;
queue creation remains atomically deduplicated per provider. An all-reserved pool
pauses new requests but keeps its automatic cycle pending, so failed verification
can reopen capacity even after green availability has passed the minimum.
Observed invalid or cooling history does not permanently occupy this target.
A cooling credential can recover and make existing green availability exceed the
configured maximum; Gateway then stops adding credentials. It does not automatically
delete or disable existing credentials to force the count down.

Manual refill works with automatic refill disabled and still obeys current
capacity. Every delivery rechecks current health, policy and route revision. A
callback exceeding remaining capacity is rejected atomically, including after a
maximum reduction. Disabled credentials are not accepted as refill results.
Per-process provider ordering, cross-process SQLite provider locks / Redis provider
admission leases, and route revision CAS protect concurrent creation and delivery.
Failed queue tasks release their outstanding reservation; automatic retries occur
on the existing bounded worker cadence, not in a tight retry loop. One transport
batch is at most 10000 credentials; this is a request-resource bound rather than a
business limit on configured capacity.

## Provider-local storage

`credential_storage_path` is an absolute local or mounted-filesystem directory for
this provider's refill JSON material. `credential_archive_path` is the provider's
recoverable archive directory. Empty/missing fields retain the existing defaults
under the global credential folder-sync root. Neither field implicitly migrates
existing files when changed. URLs such as HTTP or S3 are rejected because Gateway
has no corresponding credential storage protocol.

Refill-task `folder_sync` now reads only the task provider's directory. Each
`relativePaths` entry names one JSON file relative to that directory; each file
contains one existing `ProviderCredentialYaml` object. Empty paths select immediate
JSON children with a bounded directory scan. Paths cannot escape the directory or
traverse symbolic links. Each file is limited to 1 MiB, the delivery to 4 MiB, and
selected paths to 128. Cancelled folder reads are bounded read-only work and never
commit route changes after the request future is dropped.

This intentionally repairs the old refill-task behavior, which ignored
`relativePaths` and invoked a global PostgreSQL importer across providers. Clients
must supply provider-relative paths and the route credential JSON shape. The
separate global folder synchronization feature is unchanged. SQLite prepares
folder delivery as stable credential payloads before committing, preserving its
existing crash-retry ownership.

Archive counts and purge operations validate the archive envelope and exact provider ID;
shared directories retain other providers and ordinary credential JSON. Empty configured
directories are retained.

Prune decisions are discarded if any route revision changed while the driver was
checking, so an old failure cannot delete newly repaired credentials under the same
ID. Pruning still writes archive JSON before removing route credentials, unless the
existing permanent-delete switch was explicitly enabled. Archives remain plaintext
JSON written with restrictive file permissions on Unix. The existing
`credential_storage_password` remains a redacted secret intended for an external
worker; it does not encrypt archives and is not advertised as an implemented cloud
storage or archive-password protocol.

## Validation evidence

The focused isolated Rust harness directly includes the production schema, capacity,
availability SQL, archive/path, folder-delivery and callback-admission modules.
It exercises capacity/hysteresis, real SQLite health aggregation, unknown reservations,
atomic overflow rejection, duplicate delivery, one-slot concurrency, path containment,
symlinks and file size limits. It is not a full Gateway integration build or a real
multi-instance Redis end-to-end test. Full repository checks require the pinned
Rust toolchain and complete dependency cache in CI.
