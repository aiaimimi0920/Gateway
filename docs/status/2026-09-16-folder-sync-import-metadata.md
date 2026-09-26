# Folder-sync import metadata, 2026-09-16

This checkpoint is verified. Import bookkeeping now reads credential metadata
without hydrating unused old payloads. Four real PostgreSQL/Redis regressions
failed before the fix and pass after it; the seven-case database suite is 7/7.
The original folder-sync library remains 116/116 with six unchanged ignored cases.
The overall optimization and release goal remains active.

## Implemented boundary

A crate-local database projection selects nine fields: identity, label, status,
source kind/path/hash, sync mode/state and archived timestamp. It retains all rows
and the existing created_at ASC ordering. Import and both deletion passes use this
same snapshot. There is no placeholder payload or change to public full views.

Import's account and source-path maps now borrow entries from their owned vectors.
This removes the previous account view/payload clones and credential view clones
for lookup construction. The new query also avoids selecting credential payload
columns and avoids object-storage hydration during this initial lookup. No measured
latency or total-memory improvement is claimed. Accounts are still hydrated.

Same-hash files keep the exact old row, including a missing payload or prior sync
error. Changed files can repair a missing old inline payload using the existing
update operation, preserving credential identity, label and status. Unrelated
missing payloads no longer stop import. Missing-file deletion can use its original
eligibility rules and clean DB/Redis without first reading unused payload content.

The existing mutation functions, storage cleanup, file ordering, path normalization,
provider selection and deletion predicates are unchanged. Public credential lookup
and export still require actual payloads and fail when they are missing. Import
success does not certify unrelated credential health or guarantee a Both run's
subsequent export success. Native archive timestamps replace formatted timestamps
in the private projection; the existing optionality checks remain exact.

## Fresh evidence

The preceding management-deletion scope, all 135 bound evidence hashes, 1878 inputs,
22 assets, five published documents and both repository HEAD/status sets were
revalidated before edits. The new baseline uses unchanged production source.
Library: 116 passed, six ignored, in 1.35 seconds. Real PostgreSQL/Redis: three
passed and four failed in 6.51 seconds. All four failures were the old
Provider credential payload missing conflict at the import lookup boundary.

The frozen candidate library passes 116/116 with the same six ignored identities
in 3.57 seconds. All 122 normalized test identities/results are unchanged. The
unchanged seven database tests pass 7/7, zero ignored, in 1.70 seconds. They prove:

- Successful create, exact source metadata and hashes, same-hash whole-row stability,
  changed-file updates with preserved label/status and cleared sync error.
- Export reconstruction, semantic payload/metadata, persisted export hash/state,
  and byte-identical second export. Returned status equals persisted Redis JSON.
- Unrelated missing-payload independence, same-hash skip without repair, and
  changed-file repair of the same credential identity.
- Last-created duplicate-path selection; only that row changes. Equal timestamp
  ordering remains as unspecified as the original SQL.
- Missing-file deletion and four runtime-key removals, retaining pending, manual,
  timestamp-archived and status-archived rows and their runtime keys.
- Public full lookups and export retain missing-payload errors and preserve files.

These are public core sync API tests; HTTP route, watcher event and remote object
storage success are not established by this suite. The test target is ignored by
default and was explicitly executed with --ignored against guarded fixtures.

Serialized gates passed:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_database -- --ignored --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <nine scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

Filesystem deletion is 9/9; checker tests 19/19. Both repositories' staged and
unstaged diff checks pass. Library/runtime warning sets match their baselines.
Complete source comparison proves only the intended reexports, four import
substitutions and five deletion type substitutions. All original deletion test
bodies remain exact; only their local metadata fixture replaces the shared full
view constructor. All three new database test files remain byte-frozen.

Both real runtime phases created one PostgreSQL and one Redis container. All four
containers and anonymous volumes were removed; zero owned temporary roots remain.
All 45 pre-existing containers retain their identity/state. Normal tests drop their
schemas; panic-time schemas are removed with the guarded test-only container.

The first native admission rejected foreign Cargo/rustc processes; they exited
before resumption and none was killed. A shell observer quoting failure and the
source-proof reexport-order correction are recorded. Official rustfmt placed the
public reexport before the crate-local one; the proof was corrected to that order.
No production/test correction occurred after candidate freeze, and no successful
native gate was replayed.

## Size and scope

| File | Before effective lines | After |
| --- | ---: | ---: |
| `src/db/mod.rs` | 417 | 420 |
| `src/db/provider_credentials.rs` | 99 | 103 |
| `src/db/provider_credentials/import_metadata.rs` | new | 31 |
| `src/provider_credential_folder_sync/import.rs` | 133 | 133 |
| `src/provider_credential_folder_sync/deletion.rs` | 209 | 209 |
| `src/provider_credential_folder_sync/deletion/tests.rs` | 271 | 289 |
| `tests/provider_credential_folder_sync_database.rs` | new | 214 |
| `tests/provider_credential_folder_sync_database/schema.rs` | new | 12 |
| `tests/provider_credential_folder_sync_database/harness.rs` | new | 107 |

All nine owners remain below 500. Strict scans 2177 files: 11 hard, 20 mandatory,
39 soft; its expected exit is 1. All 31 above-700 files retain exact hashes/counts.
Structural clearance remains 114/145 (78.6%). The source/test union is 1882 with
1873 unchanged neighbors and 22 unchanged web/Tauri assets.

Gateway HEAD remains 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d; after the five
document publication, its preserved state is 194 modified, one unstaged deletion,
two staged deletions and 2425 untracked files. Neuro HEAD remains
bf818f0324024634bc890585efb78cc8e603d11a, with ten modified and 182 untracked.
No dependency, checker policy, baseline, exception, staging or commit change.

The previous lane's reviews.md incorrectly described payload_content_type as only
an object-storage local. Credential INSERT/UPDATE actually name this SQL column.
The old deletion-only schema was sufficient for its accepted tests; the new schema
adds the column for real mutations without editing old hash-bound evidence.

## Remaining work

Database row counts, account hydration, aggregate parsed memory and retained path
bounds remain open. Native deadlines/full application drain/status ordering,
blocking-pool fairness, file replacement/hard-link/cross-process races and complete
provider/UI/Docker/release acceptance remain open. The overlap of explicit-delete
and delete-missing passes over the same old snapshot is a concrete next audit.
S06 source/cursor/final build stays reserved under GWP-20260912-01. Persistent
4200 and Neuro/release/Gateway are unchanged; no new release is claimed.

Evidence: target/effective-line-evidence/20260916-folder-sync-import-metadata/.
Scope SHA-256: 1d4c45f68730f1c90ca5ebb7ecc5f0a949b13937d0fdac76bd3630a2ba0257d6.
[Lane](../plan/parallel-lanes/folder-sync-import-metadata.md).
