# Folder-sync guarded PostgreSQL and Redis integration, 2026-09-19

The disposable guarded integration path is now verified without touching the
existing Gateway, PostgreSQL, Redis, or object-store containers. The fixture uses
random container names, random loopback ports, a unique run guard, and per-test
PostgreSQL schemas.

## Verification

- `provider_credential_folder_sync_database`: 14 passed, 0 failed.
- Guarded status-store Redis suite: 8 passed, 0 failed, including the legacy
  snapshot fallback and tagged-key promotion regression.
- Guarded runtime Redis suite: 23 passed, 0 failed, including the tagged-key
  convergence and malformed-status startup fallback regressions.
- A combined status-store then runtime run passed after explicitly clearing the
  shared status and enabled keys between binaries.
- Every disposable PostgreSQL/Redis container was removed in `finally`; no
  `gateway-folder-pg-*` or `gateway-folder-redis-*` container remains.
- Existing Gemini warnings remain unrelated to this integration path.

## Filesystem hard-link follow-up

The folder-sync containment owner now rejects descendant regular files with
more than one filesystem link before import, read, export, or deletion can use
the pathname. Unix uses `MetadataExt::nlink`; Windows opens the existing file
and reads `BY_HANDLE_FILE_INFORMATION::nNumberOfLinks`, failing closed when the
link count cannot be inspected. This prevents an in-root hard-link leaf from
overwriting material outside the configured root.

- Filesystem focused tests: 26 passed, 0 failed on Windows.
- Watcher tests: 41 passed, 0 failed.
- Deletion integration: 9 passed, 0 failed.
- Complete folder-sync library at the final checkpoint: 140 passed, 0 failed,
  14 ignored.

The hard-link proof is a static link-alias guard, not a claim of atomic
handle-relative access. Concurrent replacement between validation and I/O,
Redis Cluster deployment proof or arbitrary legacy-writer coordination,
shared blocking-pool fairness, Unix/macOS backends, remote S3 success/read, and
full provider/UI/Docker/release acceptance remain open.

The database fixture proves the current minimal schema and folder-sync mutation
contracts against real PostgreSQL and Redis connections. It does not prove the
production compose schema, remote S3 success/read, Redis Cluster deployment
proof, arbitrary legacy-writer coordination, shared blocking-pool fairness,
filesystem replacement races, Unix/macOS behavior, or full provider/UI/Docker/
release acceptance.
