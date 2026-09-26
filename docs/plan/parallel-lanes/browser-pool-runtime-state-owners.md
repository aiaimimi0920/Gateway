# Browser-pool runtime-state ownership

Owner: resumed coordinator. State: accepted at 2026-09-14 00:53:36.239 UTC.
Entry 10,333 -> 10,147; extracted owner 191 effective lines. Paired Node suites
pass 50/50, including real S3 SDK loopback mirroring/cache/error behavior; paired
package contracts pass 1/1. Source/syntax/checker/ratchet and both Git checks pass.
All gates are terminal and storage fixture census is zero. The 700-input union
preserves 697 neighbors and web assets. Strict: 1,857 scanned, 16 hard,
24 mandatory, 40 soft; 40 above 700, clearance unchanged at 105/145 (72.4%).

Evidence: target/effective-line-evidence/20260914-browser-pool-runtime-state-owners/scope.json.
SHA-256: 4c9e5c58e8c4267671f733de8431565f0adbf0132b9d5926cdc9c46c2be10f23.
[Checkpoint report](../../status/2026-09-14-browser-pool-runtime-state-owners.md).

Accepted preparation/design:
Predecessor scope: target/effective-line-evidence/20260914-browser-pool-config-owners/scope.json.
SHA-256: d61f43870ef049079978bc396c7ff70af2a294dc998d548f272560ba7bb816c5.

Move six functions and their objectStorageClient cache into adjacent
gemini-canvas-browser-pool-runtime-state.mjs: resolveObjectStorageConfig,
getStorageRoot, getS3Client, mirrorRemoteRuntimeStateObject,
looksLikePersistentBrowserProfileDir and resolveRuntimeStateSource. Root imports
getStorageRoot and resolveRuntimeStateSource. Preserve function bodies, cache
reuse, environment precedence, local/profile classification, remote mirroring,
fixture fallback, error codes and file-write ordering. No callback/factory is
needed: exact source has no root logger or fixture-predicate dependency.

Root keeps profile cloning/cleanup, context creation, the fixture-policy decision
and startup storage-directory creation. The extracted module owns the existing
single process-level S3 client; no client is initialized at import time. No
path/security policy, credential refresh, response-size bound or cleanup API is
added as part of this structural move. These existing behaviors are not hardened
by this extraction.

Before production edits, add one private fixture export and frozen tests for
storage-root precedence, actual local file/directory classification, missing
configuration and fixture fallback. A bounded loopback S3-compatible server
exercises real SDK downloads, cached-client reuse, HTTP errors and disk contents
with synthetic credentials. Tests restore environment, destroy only their owned
SDK clients, close the owned listener/connections and remove contained temporary
roots. Never read live credential files or connect to external storage.

All extracted owners must be at most 500 effective lines. Run paired complete
browser-pool suites/package contracts, source/encoding/syntax, checker, ratchet,
strict and both Git checks. S18 remains incremental; no production release or
live browser is launched. S06/final build transfer remains GWP-20260912-01.
