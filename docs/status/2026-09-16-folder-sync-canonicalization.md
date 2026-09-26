# Folder synchronization canonicalization ownership, 2026-09-16

S09 now has a dedicated pure naming owner. The legacy folder-sync root decreases
from 5543 to 5366 effective lines; canonicalization.rs has 186. All six original
functions, their callers and the full original test module are preserved. Fresh
library tests pass 72/72, with the same six ignored identities. The full optimization
and release plan remains open.

## Ownership and preserved behavior

The new src/provider_credential_folder_sync/canonicalization.rs owns family,
service-provider, surface and credential-material canonicalizers, the service /
surface lookup key, and ASCII component sanitization. It depends only on the
standard library; it does not call back into its parent. Six pub(super) functions
replace the original private definitions, with explicit imports in the parent.
No public API, routing payload, HTTP/Redis schema or filesystem policy changes.

Alias tables, match ordering, unknown-surface family fallback, material underscores,
lookup-key separators, empty results and sanitizer behavior remain exact. Sanitizer
still lowercases ASCII alphanumerics, replaces other characters with hyphens and
collapses empty components. This is naming logic; containment stays with the I/O
owners. No matching rule or provider-specific parser was redesigned.

Whole-source reconstruction checks the exact root after removal and minimal wiring,
and the exact extracted child after pub(super) visibility and official formatter
signature reflow. The complete original root test module is byte-preserved. All
other captured source/test files, including watcher epochs, status ownership,
deletion, backend callbacks and database operations, retain their hashes.

The root drops 177 effective lines and remains legacy debt. The new owner is below
500; no exception is needed. The combined count grows by nine lines from module /
import wiring and the formatter's longer lookup-key signature. This checkpoint
improves ownership and review scope; it makes no latency or allocation claim.

## Verification and evidence

The before gate reuses the immediately preceding accepted disable-epoch library
result. Before editing, its scope, publication, source/test/assets, library receipt,
log, snapshot, Git HEADs and porcelain status hashes were verified. That successful
unchanged baseline was not rerun. The candidate executes the same command freshly:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1

Result: 72 passed, zero failed, six ignored, 3038 filtered, finished in 0.18 seconds.
The verifier requires exactly 78 emitted identities, partitioned into 72 passing
and six ignored, then compares every identity/result to the predecessor. No test
was added, removed, relocated or weakened for this behavior-preserving extraction.

Fresh serialized gates also pass:

    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check src/provider_credential_folder_sync.rs src/provider_credential_folder_sync/canonicalization.rs
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

Checker tests pass 19/19. Source reconstruction, UTF-8/no-BOM checks, and Gateway /
Neuro staged and unstaged git diff --check all pass. Native admission and terminal
process guards pass; no foreign process was killed. Source/tests and 22 web/Tauri
assets were hash-checked around each gate. No successful native gate was replayed.
Existing compile warnings remain: three library warnings and one lib-test warning.

Strict exits 1 for remaining debt: 2125 scanned / 12 hard / 20 mandatory / 39 soft.
There are still 32 files above 700; only the root decreases, while the other 31
retain identical hashes and counts. Clearance remains 113/145 (77.9%). The source
union contains 1830 files, with 1828 unchanged neighbors and 22 unchanged assets.
Checker, policy, baseline, exceptions and dependencies are unchanged.

Two independent read-only reviews found no source regression. The evidence review
led to explicit identity counts, final full Git status-set comparison and precise
wording about signature formatting. A first combined patch was rejected before
writes; smaller boundary-checked patches applied successfully. The first closing
scope capture incorrectly looked for the compliant child in the report's debt-only
files array. Its verifier and failure are preserved; only evidence processing was
corrected to use the official lexer. Source, tests and passed gates were unchanged.

Evidence: target/effective-line-evidence/20260916-folder-sync-canonicalization/.
scope.json observedAt: 2026-09-15T22:02:41.787Z. Scope SHA-256:
29827c05743972c8ce0be617ce8d9ff025dd59d2005ede9a25b17a2b5cf26d53.
All captured inputs, scripts, reviews, failures and gate receipts are hash-bound.
publication.json verifies the five final documents and exact repository deltas.

Gateway retains 194 modified, one unstaged deletion, two staged deletions and 2355
untracked entries, including this lane's one source and two documents. Neuro retains
10 modified and 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a respectively. No staging, commit, dependency,
profile, persistent service or release change occurred.

## Remaining work

The prior [disable-epoch checkpoint](2026-09-16-folder-sync-disable-epochs.md) was
published and verified before this extraction began. Its Redis runtime 10/10 is
retained historical evidence; Redis/Docker/database runtime tests were not rerun
for this pure naming move. No new PostgreSQL or provider success is claimed.

Caller input lengths, empty/colliding sanitized names, Unicode loss and proportional
sanitizer allocations remain existing behavior. Endpoint Git audits cannot prove
the absence of a hypothetical transient external added-and-removed file during a
gate; all captured inputs were checked before/after, and final path sets are exact.
I/O deadlines, shared status priority, backend shutdown, successful database sync,
remaining root migration and full provider/runtime/UI/Docker/release acceptance
remain open. S06 source/cursor/final build stays reserved under GWP-20260912-01.
Persistent target stays 4200; release root stays Neuro/release/Gateway. The overall
optimization goal remains active.
