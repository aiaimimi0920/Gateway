# Folder-sync transient account hydration bound, 2026-09-18

This incremental checkpoint is verified for source and local tests. It adds a bounded
admission check for the one selected provider account that folder-sync must hydrate; the
overall optimization and release goal remains active.

## Implemented boundary

`get_provider_account_for_folder_sync` is private to the folder-sync database path. It
recovers expired account cooldowns, opens a short PostgreSQL transaction, locks the target
row, and first selects only `octet_length(payload_inline::text)::bigint`. An inline value
above the standard 32 MiB object-storage limit is rejected with
`provider_credential_folder_sync_account_payload_too_large` before the full row is read.
The transaction also prevents a concurrent account replacement between the preflight and
the row hydration.

The public account lookup remains unchanged. A null inline payload with an object key still
uses `GatewayObjectStorage::read_json`, whose existing bounded reader enforces the same
standard object limit and fail-closed errors. The scan cache continues to apply its
conservative retained-byte and LRU limits after hydration.

The regression fixture creates a selected 32 MiB-plus inline JSON payload, writes a changed
credential file, and enables missing-file deletion. The run returns the stable size error,
creates no credential mutation, deletes no retained row, and preserves all four credential
runtime keys. The test source compiles in the current environment, but its guarded fixture
cannot initialize without the three dedicated environment variables.

## Verification

Fresh checks:

    cargo test --offline --locked --lib provider_credential_folder_sync:: -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_database --no-run
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --check <eight scoped source/test files>
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts
    git diff --check
    git diff --cached --check

Results are 126 passed, zero failed and 12 ignored for the folder-sync library, and the
object-storage focused group passes 19/19. All-targets, scoped formatting, checker 19/19
and ratchet pass. The ratchet inventory is 2,183 files
with 11 hard, 20 mandatory and 39 soft inherited entries. The ignored database command
enumerates 14 tests but all stop at fixture initialization because guarded variables are
absent; it is not a code regression result.

## Size and scope

| File | Effective lines |
| --- | ---: |
| `src/db/provider_accounts/lookup.rs` | 134 |
| `src/db/provider_accounts.rs` | 104 |
| `src/db/mod.rs` | 424 |
| `src/object_storage/body_limit.rs` | 186 |
| `src/object_storage.rs` | 85 |
| `src/provider_credential_folder_sync/import.rs` | 151 |
| `src/provider_credential_folder_sync/limits.rs` | 47 |
| `tests/provider_credential_folder_sync_database.rs` | 489 |

All scoped files are below the 500-line acceptable boundary and pass UTF-8 no-BOM and
trailing-whitespace checks. Existing dirty worktrees, port 4200 and `Neuro/release/Gateway`
were not modified.

## Remaining work

Selected remote object success/read proof, aggregate parsed account/value peak bounds,
native deadlines/full drain/status races, shared-pool fairness, filesystem
replacement and hard-link/reparse races, Unix/macOS behavior and full provider/UI/Docker/
release acceptance remain open. Aggregate normalized path retention is covered by the
2026-09-18 follow-up checkpoint. S06 source and final native-build ownership stay reserved.

Evidence: `target/effective-line-evidence/20260918-folder-sync-transient-account-hydration/`.
[Lane](../plan/parallel-lanes/folder-sync-transient-account-hydration.md).
