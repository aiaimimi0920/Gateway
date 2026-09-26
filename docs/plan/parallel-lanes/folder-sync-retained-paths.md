# Folder-sync retained source-path bounds

Verified on 2026-09-17. Folder import now rejects a credential metadata snapshot
whose stored source paths exceed 32 MiB before it fetches the full snapshot or
starts either deletion phase. The stable error code is
`provider_credential_folder_sync_source_path_limit`. The existing 100,000-row
sentinel remains independent and keeps its existing database-limit code.

The SQL preflight applies the same `created_at ASC` order and `max_rows + 1`
window as the metadata fetch. A second saturating byte check runs over fetched
rows, so growth between the preflight and fetch also fails before mutation. The
real PostgreSQL/Redis regression creates more than 32 MiB of source paths,
enables missing-file deletion, and proves that the retained credential and all
four runtime keys survive the failure.

`CredentialPathIndex` owns the metadata snapshot and stores one normalized
`Arc<str>` key for each usable source path. `SourcePathInterner` reuses equal
normalized keys across metadata and observed filesystem paths. Duplicate stored
paths preserve the existing last-created-row lookup behavior, while iteration
keeps database order for deletion. Both deletion phases consume indexed entries,
so they do not repeatedly normalize every stored path. The old pure metadata
predicates remain test-only to retain their focused behavioral coverage.

The complete folder-sync unit group passes 121/121 with 12 external-service
tests ignored. The final source snapshot passes the 13/13 guarded database suite,
including the new pre-mutation byte-limit regression. All-targets, scoped
formatting, checker 19/19, ratchet and Git diff checks pass. Existing warnings and
global formatting differences remain confined to the reserved S06 scope.

The new path owner is 122 effective lines. All eight scoped production/test
files are at most 443 effective lines. Ratchet scans 2,182 files and passes.
Strict remains expected-red with 11 hard, 20 mandatory and 39 soft entries.
Scoped files are UTF-8 without BOM, and the guarded fixture reports zero owned
containers after cleanup.

The database preflight bounds stored source-path text for one import snapshot.
Watcher pending deletion intent still retains distinct paths until the next
successful run and needs its own byte boundary. Single-account transient
hydration, selected remote object-storage success/read limits, native deadlines
and complete drain, status ordering, blocking-pool fairness, filesystem races,
Unix/macOS behavior and complete provider/UI/Docker/release acceptance remain
open. S06 stays reserved and the overall goal remains active.

Evidence: `target/effective-line-evidence/20260917-folder-sync-retained-paths/`.
Scope SHA-256: `0cedc4629ebc72c4393f8025e30dd59f4b4d0de4411481c0bc13b8a7ae533dad`.
[Report](../../status/2026-09-17-folder-sync-retained-paths.md).
