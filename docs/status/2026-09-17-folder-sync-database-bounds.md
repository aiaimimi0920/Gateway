# Folder-sync database bounds, 2026-09-17

This incremental checkpoint is verified. Folder import now has finite provider-account
and credential-metadata database snapshots, avoids full account payload reads until a
changed file selects an account, and fails before deletion when either snapshot would
be truncated. The overall optimization and release goal remains active.

## Implemented boundary

`list_provider_account_import_metadata` selects ten classification fields and orders
by `created_at ASC`. It requests `max_rows + 1`, accepts 4,096 rows and returns
`provider_credential_folder_sync_database_limit` on the sentinel row. It does not read
object payloads. The only payload-derived projected field is `baseUrl`/`base_url` from
inline JSON.

`list_provider_credential_import_metadata` applies the same sentinel pattern at
100,000 rows while retaining all nine fields needed by duplicate-path selection,
updates and both deletion passes. Its `created_at ASC` order and last-wins map behavior
remain unchanged. Import cannot run explicit-delete or delete-missing against a
truncated snapshot.

Account classification maps use the bounded projection. Canonical account create and
update paths derive and persist protocol family/profile from the full input payload
independently of whether that payload is stored inline or as an object, so supported
object-backed records retain scalar classification metadata. Once a path selects an
account, import checks the existing source hash first. Only changed/new material
hydrates the full account; a per-run ID map reuses that view for later files.
Normalization and mutations still receive the real full account view.

An unselected object-backed row with a missing object does not block an unrelated
import. Same-hash material succeeds even when the selected account payload is missing.
A changed file selecting that account returns the original 409 conflict and creates no
credential. This is fail-closed behavior; no placeholder payload or provider fallback
was added. Public account/credential lookup and export contracts are unchanged.

## Runtime evidence

The guarded PostgreSQL/Redis database target passes 12/12 in 7.78 seconds after its
Cargo build. Five new cases prove:

- a same-hash file skips before selected-account payload hydration;
- an unselected object-backed account is not hydrated;
- scalar classification selects the intended account and missing full payload fails;
- 4,096 accounts succeed, 4,097 fail with the stable code/message before deletion;
- 100,000 credential rows succeed, 100,001 fail before deletion.

Both overflow cases enable delete-missing after the exact-bound run, retain a
deletion-eligible credential, and retain all four associated Redis runtime keys. The
seven previous database cases also remain green, covering create/update/skip/export,
full state and hashes, missing old payload independence/repair, duplicate source-path
last-wins, deletion eligibility/cache cleanup and public lookup/export failure.

Additional fresh gates:

    cargo test --offline --locked --lib provider_credential_folder_sync -- --test-threads=1
    cargo test --offline --locked --lib provider_credential_folder_sync::import::tests -- --ignored --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --check <nine scoped files>
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts
    git diff --check
    git diff --cached --check

The library result is 116 passed and 12 ignored. The six guarded import/deletion phase
tests pass 6/6, filesystem deletion passes 9/9, checker tests pass 19/19, and the
ratchet passes. All-targets passes with three pre-existing Gemini warnings; the library
test has one of those warnings. Full `cargo fmt --all -- --check` returns 1 only for
`gemini_canvas_runtime_mirror.rs` and its test, both in the reserved S06 scope. The
scoped formatter passes all nine files in this checkpoint.

Each guarded run used a random PostgreSQL container and Redis container with a unique
ownership label and anonymous volumes. The database run's post-test wrapper first hit
a Docker Go-template quoting error; the overlap run's wrapper used the unsupported
PowerShell 5.1 `Select-Object -Reverse` option. Both errors occurred after all tests
passed. Recovery inspected exact container IDs, names, labels and `AutoRemove`, then
removed all four containers and four anonymous volumes. No container with either
ownership label remains, and no unrelated container was targeted.

## Size and scope

| File | Effective lines |
| --- | ---: |
| `src/db/provider_accounts/import_metadata.rs` | 44 |
| `src/db/provider_credentials/import_metadata.rs` | 41 |
| `src/provider_credential_folder_sync/accounts.rs` | 268 |
| `src/provider_credential_folder_sync/classification.rs` | 413 |
| `src/provider_credential_folder_sync/import.rs` | 147 |
| `src/provider_credential_folder_sync/limits.rs` | 44 |
| `tests/provider_credential_folder_sync_database.rs` | 396 |
| `tests/provider_credential_folder_sync_database/harness.rs` | 107 |
| `tests/provider_credential_folder_sync_database/schema.rs` | 12 |

All scoped owners remain below 500. Ratchet scans 2,180 files and passes. Strict scans
the same files with 11 hard, 20 mandatory and 39 soft entries; its expected exit is 1
because inherited debt remains. Thirteen scoped source/test inputs are UTF-8 without
BOM. Gateway HEAD remains `4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`; Neuro
HEAD remains `bf818f0324024634bc890585efb78cc8e603d11a`. The dirty worktrees are
preserved. There is no dependency, checker policy, baseline, exception, staging,
commit, release or persistent service change.

## Remaining work

Selected remote object-storage success is not proved by these local missing-object
contracts. Legacy rows written outside canonical account mutations can lack reliable
scalar classification metadata and need a separate compatibility decision. Aggregate
parsed payload memory and retained source-path memory remain bounded only indirectly
by row/file limits and need explicit accounting.

Native operation deadlines, complete shutdown drain, status ordering, blocking-pool
fairness, concurrent replacement, hard-link/reparse and cross-process races, Unix/macOS
behavior and complete provider/UI/Docker/release acceptance remain open. S06 source,
cursor and final native-build ownership remain reserved. The persistent service on
port 4200 and `Neuro/release/Gateway` were not touched.

Evidence: `target/effective-line-evidence/20260917-folder-sync-database-bounds/`.
Scope SHA-256: `5cab763a22ffbf1d460d101b5a6f0ba78fc3404a0582d1dd55992f789af5641b`.
[Lane](../plan/parallel-lanes/folder-sync-database-bounds.md).
