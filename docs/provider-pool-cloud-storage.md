# Provider pool cloud storage contract

Each provider can set `credential_storage_connection` (refill material) and
`credential_archive_connection` (recovery archives) independently. Missing objects
preserve existing `credential_storage_path` / `credential_archive_path` and shared
filesystem defaults. Selecting a connection does not migrate or export live accounts.

Connection objects have a `type` discriminator:

- `local`: optional absolute `path`; empty/missing path retains legacy defaults
- `webdav`: `endpoint`, relative `directory`, optional `username` and secret
  `password`, optional `allow_insecure_http` (default false)
- `s3`: S3-compatible HTTPS `endpoint`, `bucket`, explicit `region` (R2: `auto`),
  relative `prefix`, secret `access_key_id` and `secret_access_key`, optional secret
  `session_token`, optional `allow_insecure_http` (default false)

S3 uses path-style addressing and credentials scoped to this connection, never
ambient AWS credentials. WebDAV supports anonymous access when username is empty;
a password requires a username. TLS certificate verification stays enabled.
HTTP requires explicit opt-in because it exposes credentials and archive content.
Redirects are never followed. URL userinfo, query authentication, fragments, path
traversal and encoded path escapes are rejected. URLs placed in legacy local-path
fields remain invalid; select the protocol explicitly.

## Supported safety capabilities

| Selected service | Read / refill | Create recovery archive | Purge archive |
| --- | --- | --- | --- |
| Local filesystem | Supported | Supported | Supported with ownership/revision checks |
| Official AWS S3 HTTPS service endpoint | Supported | Create-only conditional PUT | ETag-conditional DELETE |
| Official default R2 HTTPS account endpoint | Supported | Create-only conditional PUT | Disabled: atomic conditional DELETE unconfirmed |
| Other S3-compatible endpoint | Supported | Disabled: conditional-create support unconfirmed | Disabled: atomic conditional DELETE unconfirmed |
| WebDAV | Supported | Standard create-only conditional PUT | Disabled: deployment-specific atomic DELETE unconfirmed |

This is a current, conservative capability boundary, not a claim that WebDAV or
R2 can never support safe deletion. Neither successful HEAD/GET nor an ETag proves
that a service enforces DELETE preconditions. Unsupported purge fails before any
remote DELETE is sent, including when a server would ignore `If-Match` and return
204. There is no production capability override and no HEAD-then-unconditional
DELETE fallback. Test fixtures enable private compile-time-only capabilities
explicitly; localhost is never implicitly trusted.

Official AWS recognition is limited to HTTPS port 443, no base path, and standard
`s3.amazonaws.com`, allowlisted regional `s3.{region}.amazonaws.com`, or regional
`dualstack` service endpoints. The region allowlist follows the reviewed
[AWS endpoint table](https://docs.aws.amazon.com/general/latest/gr/s3.html); new
regions and other partitions stay unverified until separately reviewed. Bucket hostnames, custom/proxy endpoints, website
endpoints, suffix lookalikes and insecure HTTP do not qualify. The current R2
create allowlist is exactly its 32-hex-account-ID default
`{accountId}.r2.cloudflarestorage.com` HTTPS endpoint; custom and jurisdiction
endpoints are conservatively unverified in this slice.

AWS documents both [conditional creates](https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-writes.html)
and [conditional deletes](https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-deletes.html).
The [R2 compatibility matrix](https://developers.cloudflare.com/r2/api/s3/api/)
confirms conditional PutObject but does not establish conditional DeleteObject.
WebDAV creation follows the conditional-request semantics required by
[RFC 4918](https://www.rfc-editor.org/rfc/rfc4918).

A future service integration may unlock an operation only after official
service-specific evidence and, where needed, isolated nonproduction canary tests
using synthetic data demonstrate atomic precondition enforcement. That requires
a reviewed code capability change and regression tests. This feature does not
probe real stores or mutate production data to discover capabilities.

## Namespaces and worker placement

Let `providerHash` be lowercase SHA-256 hex of the exact UTF-8 provider ID.
Gateway appends these paths beneath the configured directory or prefix:

- Refill: `gateway-pool/{providerHash}/refill/`
- Archives: `gateway-pool/{providerHash}/archive/`

WebDAV appends these components beneath the endpoint's existing service path.
S3 appends them inside the named bucket. The demand API returns the effective
locations. External refill workers write one `ProviderCredentialYaml` JSON per
immediate child file in the refill namespace. Cloud `relativePaths` selects
immediate `.json` names; an empty selection lists immediate JSON children. Cloud
files do not use the global account-folder importer. Maximum selection is 128,
per-credential JSON is 1 MiB and total delivery is 4 MiB. Per-object and decoded
response bodies are capped at 4 MiB; listings cap at 1024 entries, S3 at 32 pages,
and repeated S3 continuation tokens fail. Individual operations have a 30-second
deadline. Status counts have a 5-second per-provider budget; demand listing uses
at most four concurrent providers and a 30-second overall budget.

## Archive safety and failure semantics

Archives remain plaintext JSON at the destination. The password authenticates to
storage; it does not encrypt files. The legacy `credential_storage_password`
remains a separate external-refill-worker secret and is never repurposed.

New cloud archives have version 2, the exact source route revision, provider ID,
credential ID, RFC3339 archive time,
reason `permanent_driver_rejection`, and the credential. A deterministic SHA-256
object name binds that source revision, credential ID and canonical credential
payload. It uses the same revision as the final source-removal CAS, not the earlier
driver request snapshot. Retries within one revision reuse the verified object; a
later revision must create a different object even for identical credentials.
Version-1 objects remain readable/countable/purgeable but are never reused as
proof for a new source removal. Every remote
archive creation uses `If-None-Match: *`; there is no unconditional replacement.
A 409/412 conflict or uncertain response triggers a read of the same key. A valid
matching archive is reused; a competing invalid or different record is retained
and the prune fails. Retrying first reads the same key and verifies its envelope
and payload. Unknown S3-compatible services cannot create new archives.
Source credentials are removed only after every required archive is readable and
verified and the route revision still matches; the route commit also uses CAS.
A failure or unverified write preserves source credentials. Partial archive writes
may remain for the next retry.

Count and purge accept only exact provider-owned envelopes whose names match their
contents. Other providers, ordinary credential JSON, and renamed/copied files are
retained. Purge first acquires normal provider admission, captures the source route
snapshot, and freezes a bounded candidate list before any deletion. Remote candidates
retain only key, SHA-256 and strong ETag (at most 1024 entries / 2 MiB metadata), not
all archive bodies. Local candidates freeze UUID filenames and content hashes.

Purge then commits an unchanged route document through the real durable CAS. This
advances its revision sequence and is the safety barrier: any source-removal CAS
using an earlier revision must fail. No DELETE is sent if that barrier fails or its
result is uncertain. The barrier is never rolled back. This works across instances
sharing the same route authority and does not rely on an in-memory mutex or an
unexpired admission lease. An equivalent already-committed barrier may be accepted
idempotently by more than one coordinator; each can only execute its original frozen
set. Correctness does not require a globally unique DELETE executor.
New revision-bound remote archives and new local UUID
files cannot enter the frozen delete set, including when an old DELETE completes late.

After the barrier, purge re-reads candidates, requires unchanged bytes and the frozen
strong ETag, and uses conditional If-Match DELETE with that exact version. It never
re-lists to expand its candidates. Missing/weak ETags, authorization errors, missing
objects, changed content/version, observed revision changes and conditional conflicts
fail closed. Completed deletions are not rolled back; errors report confirmed deletions
and that an unconfirmed DELETE may have completed. The UI refreshes after both success
and failure because the barrier may already be committed. Files archived concurrently
after the snapshot remain for a future explicit purge.

Current remote purge is enabled only for the official AWS S3 endpoint boundary above.
WebDAV, R2 and unknown compatible endpoints remain disabled. A deployment must not
run these v2 writers concurrently with older experimental v1 cloud writers that reuse
revision-independent keys. The v1 prototype was not released; this is a compatibility
restriction, not a claim that it was previously deployed. Instances sharing an archive
namespace must share the same durable route authority and compatible writer semantics.
For versioned AWS buckets, this removes the current object or creates a delete
marker; historical object versions are not enumerated or erased. The operation
does not promise cryptographic erasure or deletion of all recovery history.

Archive count failure returns `archivedCredentialCount: null` with
`archiveStorageError`; it is never presented as a successful zero count.
`archivePurgeSupported` and `archivePurgeUnsupportedReason` separately explain
whether safe purge is available, regardless of the current archive count.
`storageAuthConfigured` / `archiveAuthConfigured` describe the selected connection;
`storagePasswordConfigured` still describes only the legacy external-worker secret.

## Secret editing

Returned route documents omit all cloud authentication secrets. Existing secret
access grants and secret patches apply at
`/providers/{index}/credential_{storage|archive}_connection/{secretField}`.
Creating a connection and replacing its auth can occur in one atomic edit. Keep
is bound to stable provider ID, connection type and endpoint, plus WebDAV username
or S3 bucket and region. Changing that identity requires replacing or clearing
secrets. Changing type removes obsolete secret fields rather than copying auth to
the new protocol. No transport error includes server bodies or auth values.

## Test scope

All transport tests bind localhost and use synthetic credentials. They cover
WebDAV GET/PUT/PROPFIND/MKCOL/DELETE, S3 SDK SigV4 and pagination, read-back after an
uncertain write, duplicate retry, strict ownership, bounded decoded bodies,
authorization/missing objects, redirects, deadlines and conditional deletion.
Local isolated harnesses use the real AWS SDK and production policy/archive/
secret modules, but substitute cached reqwest and Axum test dependencies because
pinned wreq/Axum versions are absent locally. They are focused evidence, not the
full Gateway locked build, live cloud interoperability, multi-instance Redis or a
deployment. The pinned Rust 1.98 and exact dependency gates remain CI obligations.
