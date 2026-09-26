# Folder-sync shared enabled read projection, 2026-09-18

This checkpoint fixes the management status read path using a stale process-local
runtime value after another Gateway process changes the folder-sync override.
The public status schema and write ordering remain unchanged.

## Ownership boundary

- When `gw:{provider-credential-folder-sync}:enabled` exists, its boolean is the
  cross-process management value for the status read projection. The reader
  falls back to the pre-cluster key `gw:provider-credential:folder-sync:enabled`
  while existing data is being observed and migrated by new writers.
- The read path applies the shared value together with static folder-sync
  configuration and does not mutate the caller's process-local runtime.
- When the override key is absent, the startup-loaded runtime remains the
  fallback, preserving first-start behavior before management has initialized the
  shared key.
- Existing status writers still preserve `enabled` from an existing Redis status
  snapshot when they are observers; management writes remain the only full owner.

This closes a read-side stale-value boundary. It does not establish watcher
activation in another process, distributed serialization for independent
override/status writes, or a durable cross-key transaction.

## Verification

- Focused status tests: 9 passed, 6 ignored.
- Complete folder-sync library: 138 passed, 12 ignored.
- Guarded runtime integration target compiled with `--no-run`; its Redis tests,
  including the new cross-process read regression, remain ignored because the
  dedicated fixture environment is absent.
- `cargo check --offline --locked --all-targets`: passed with the existing three
  Gemini warnings.
- Effective-lines checker tests: 19 passed; ratchet passed with 2,184 files
  (`>1500=11`, `701-1500=20`, `501-700=39`).
- Scoped rustfmt and `git diff --check`: passed.

## Remaining work

The guarded PostgreSQL/Redis runtime suite still needs its dedicated run ID and
database URLs. Cross-process watcher convergence, same-field write precedence,
remote S3 success/read, shared blocking-pool fairness, filesystem replacement and
hard-link/reparse races, Unix/macOS behavior, and full provider/UI/Docker/release
acceptance remain open. S06 source/cursor/final build ownership stays reserved.
