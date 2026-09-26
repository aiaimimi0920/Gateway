# Folder-sync deletion overlap, 2026-09-16

This GWP-20260916-19 checkpoint is verified. Folder import retains explicit deletion
before delete-missing, but the second phase now excludes every normalized source path
covered by the explicit intent set. The same stale metadata snapshot no longer deletes
one credential twice. The overall optimization and release goal remains active.

## Implemented boundary

The import coordinator passes the existing `explicit_deleted_paths` set to the
delete-missing phase. Its eligibility predicate retains archive, status, sync mode,
materialized-copy, normalization and observed/recreated-path checks, then applies the
existing exact-or-directory-prefix matcher negatively. Near-prefix paths remain
eligible for ordinary missing deletion. No copied ID set or new public API is added.

The explicit loop, its DB then Redis ordering, counters, audit construction and error
propagation remain byte-exact. The missing loop also retains its DB then Redis ordering
and propagates genuine 404 failures. The second phase starts only after the complete
explicit phase succeeds, so excluding explicit-intent paths does not hide partial
explicit failures.

## Fresh evidence

The preceding import-metadata scope, its publication and bound evidence were validated
before this lane. A pre-candidate evidence correction adds the existing deletion unit
test file to scope using its exact accepted GWP-18 bytes and effective-line count.
The original baseline receipts remain immutable; the correction and source copy are
hash-bound. Both new database regression files stay byte-identical to baseline.

Library baseline and candidate are identical: 116/116 passed, zero failed, 12 ignored,
with all 128 identities/results exact. The six new guarded PostgreSQL/Redis tests had
four baseline passes and two failures. Both overlap cases failed at the intended stale
snapshot boundary with `Provider credential` not found after the explicit phase had
already deleted the row. The candidate passes 6/6. It proves:

- Exact explicit-then-missing phase order and one DB deletion per credential ID.
- Directory-prefix intent, duplicate source-path rows and near-prefix independence.
- Observed/recreated protection and unchanged archived/manual/pending eligibility.
- `deleted_count` counts rows while the explicit audit counts distinct paths and IDs.
- Explicit failure stops before missing and preserves all runtime keys.
- A genuine missing-phase failure after explicit success is still returned.

The unchanged public database suite passes 7/7. The deletion target passes 9/9.
Scoped rustfmt, `cargo check --offline --locked --all-targets`, checker tests 19/19,
the effective-line ratchet, and staged/unstaged diff checks in Gateway and Neuro all
pass. Strict scans 2179 files with 11 hard, 20 mandatory and 39 soft findings and
returns the expected exit 1. All 31 files above 700 retain exact hashes/counts;
structural clearance remains 114/145 (78.6%).

The source/test union is 1884 with 1879 unchanged neighbors and 22 unchanged assets.
Five scoped owners measure 136, 213, 295, 131 and 206 effective lines. Source proof
limits production changes to the new argument and the shared prefix exclusion; the
explicit loop and matcher are exact. The existing deletion tests only gain one empty
set and its required arguments.

Three real-runtime phases each created one guarded PostgreSQL and one Redis container.
All six owned containers and anonymous volumes were removed, all schemas/keys/temporary
roots were cleaned, and the same 45 pre-existing containers were preserved. Persistent
port 4200 and `Neuro/release/Gateway` were untouched.

Four independent default-role review attempts failed before repository access because
the configured model route returned HTTP 503 with no available channel. The coordinator
review found no issue and is recorded without attributing an external approval. The
first scope-verifier attempt also stopped on four unrelated new Neuro untracked files;
they were preserved and are bound as explicit parent-repository additions. Gateway's
status set remained exact.

## Remaining work

Database row/account hydration limits, aggregate parsed-memory and retained-path bounds,
native deadlines/full application drain/status ordering, blocking-pool fairness,
replacement/hard-link/cross-process races, Unix/macOS behavior and complete provider/
UI/Docker/release acceptance remain open. The 31 inherited files above 700 remain debt.
S06 source/cursor/final native build stays reserved under GWP-20260912-01.

Evidence: `target/effective-line-evidence/20260916-folder-sync-deletion-overlap/`.
Evidence key: `folder-sync-deletion-overlap`.
Scope SHA-256: `482401e6e17c6b3bd2f0402844b86d5cc15929a2f30b267b60dc0289ebc533e7`.
[Lane](../plan/parallel-lanes/folder-sync-deletion-overlap.md).
