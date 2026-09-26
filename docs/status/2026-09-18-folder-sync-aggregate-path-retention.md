# Folder-sync aggregate path retention bound, 2026-09-18

This incremental checkpoint adds a scan-level bound for normalized path ownership.
The overall folder-sync optimization and release goal remains active.

## Implemented boundary

`SourcePathInterner` owns a 64 MiB budget for distinct normalized source-path keys.
Each key is charged by allocated string capacity plus a 384-byte margin for the
interner/index/set ownership. Saturating arithmetic treats an unrepresentable sum as
over budget. Equal normalized paths reuse the existing `Arc<str>` and do not consume
the budget twice.

The same interner is used while building `CredentialPathIndex` and while walking the
observed filesystem paths. A failed admission returns
`provider_credential_folder_sync_path_retention_limit` and no index or deletion phase
receives a partial key set. After interning, the metadata row's original `source_path`
string is cleared so the retained snapshot does not keep a second copy of the path.

The existing 32 MiB database source-path preflight, 512-character path validation,
100,000 metadata-row limit and 10,000-file filesystem limit remain unchanged. The
shared keys preserve last-created duplicate lookup, database-order deletion and
explicit-delete audit behavior.

## Verification

Fresh checks:

    cargo test --offline --locked --lib provider_credential_folder_sync:: -- --test-threads=1
    cargo test --offline --locked --lib provider_credential_folder_sync::credential_paths -- --test-threads=1
    cargo test --offline --locked --lib object_storage:: -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_database --no-run
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --check <three scoped source files>
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts
    git diff --check

The folder-sync library passes 129 tests with zero failures and 12 ignored external
service tests. The dedicated path group passes 5/5. The object-storage group passes
20/20, including local write/read of the canonical provider-account object key. The
guarded database target is compiled but its runtime suite is not claimed without the
dedicated PostgreSQL/Redis environment variables. Existing Gemini/S06 warnings remain
outside this lane.

## Size and scope

The production owner remains `src/provider_credential_folder_sync/credential_paths.rs`
with one responsibility: shared normalized path ownership and aggregate admission.
The import wiring and error constructor are small incremental changes. No dependency,
checker policy, baseline, exception, release artifact or persistent service changed.

## Remaining work

The aggregate bound accounts for shared normalized key storage and a conservative
per-key margin; it is not a whole-process RSS proof and does not bound notify's native
event allocation before callback creation. The local provider-account object path is
covered, while remote S3 success/read proof, native deadlines/full drain/status races, shared-pool fairness, replacement and
hard-link/reparse races, Unix/macOS behavior and full provider/UI/Docker/release
acceptance remain open. Port 4200 and `Neuro/release/Gateway` were not touched.

Evidence: `target/effective-line-evidence/20260918-folder-sync-aggregate-path-retention/`.
[Lane](../plan/parallel-lanes/folder-sync-aggregate-path-retention.md).
