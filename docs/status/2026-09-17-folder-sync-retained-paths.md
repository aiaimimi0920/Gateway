# Folder-sync retained source-path bounds, 2026-09-17

This incremental checkpoint is verified. Credential metadata row bounds and the
hydrated-account LRU remain in place, while source-path retention now has a
database preflight and a shared normalized-key owner. The overall optimization
and release goal remains active.

## Implemented boundary

`list_provider_credential_import_metadata` accepts both the 100,000-row limit and
a 32 MiB source-path byte limit. It first queries an ordered sentinel window for
row count and `octet_length(source_path)`. Either overflow returns before the
full metadata fetch and before explicit or missing-file deletion. A post-fetch
saturating byte sum closes growth between the two queries. Source-path overflow
uses `provider_credential_folder_sync_source_path_limit`; row overflow keeps
`provider_credential_folder_sync_database_limit`.

`CredentialPathIndex` owns metadata rows in creation order and records their
normalized source paths. A shared `SourcePathInterner` returns `Arc<str>` keys
for both stored metadata and observed files. The path lookup keeps last-row-wins
behavior for duplicates, and deletion iterates in the original database order.
Production deletion predicates consume the indexed path instead of normalizing
the credential string once per deletion phase.

## Verification

Fresh final-source gates:

    cargo test --offline --locked --lib provider_credential_folder_sync:: -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_database -- --ignored --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --check <eight scoped source/test files>
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts
    git diff --check
    git diff --cached --check

The folder-sync library result is 121 passed, zero failed and 12 ignored. This
includes two focused path-index tests. The real guarded PostgreSQL/Redis target
passes 13/13 on the final source snapshot. The new regression exceeds the exact
32 MiB source-path budget, receives the stable error, and proves the deletion
candidate and its four Redis keys remain. The fixture ownership label reports
zero remaining containers.

All-targets passes with the three pre-existing Gemini/S06 warnings. Checker tests
pass 19/19 and the ratchet passes. The strict audit has the expected exit 1 from
inherited debt, with 2,182 files, 11 hard, 20 mandatory and 39 soft entries.
Global formatting remains expected-red only for the two reserved S06
runtime-mirror files; scoped formatting passes.

## Size and scope

| File | Effective lines |
| --- | ---: |
| `src/db/provider_credentials/import_metadata.rs` | 85 |
| `src/provider_credential_folder_sync.rs` | 61 |
| `src/provider_credential_folder_sync/account_cache.rs` | 224 |
| `src/provider_credential_folder_sync/credential_paths.rs` | 122 |
| `src/provider_credential_folder_sync/deletion.rs` | 251 |
| `src/provider_credential_folder_sync/import.rs` | 146 |
| `src/provider_credential_folder_sync/limits.rs` | 45 |
| `tests/provider_credential_folder_sync_database.rs` | 443 |

All scoped files are below 500 and UTF-8 without BOM. Gateway HEAD remains
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`; Neuro HEAD remains
`bf818f0324024634bc890585efb78cc8e603d11a`. Existing dirty worktrees are
preserved. No dependency, policy, baseline, exception, staging, commit, release
or persistent service state changed.

## Remaining work

The new byte preflight bounds stored credential source-path text. Watcher mailbox
and pending work still retain a `HashSet<String>` of distinct deletion intent and
need a separate finite byte boundary that preserves fail-closed deletion
semantics.

The single hydrated account currently in use, selected remote object-storage
success/read limits, native operation deadlines, complete shutdown drain, status
ordering, blocking-pool fairness, concurrent replacement, hard-link/reparse and
cross-process races, Unix/macOS behavior and complete provider/UI/Docker/release
acceptance remain open. S06 source, cursor and final native-build ownership remain
reserved. Port 4200 and `Neuro/release/Gateway` were not touched.

Evidence: `target/effective-line-evidence/20260917-folder-sync-retained-paths/`.
Scope SHA-256: `0cedc4629ebc72c4393f8025e30dd59f4b4d0de4411481c0bc13b8a7ae533dad`.
[Lane](../plan/parallel-lanes/folder-sync-retained-paths.md).
