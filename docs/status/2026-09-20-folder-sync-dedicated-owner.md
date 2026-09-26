# Dedicated owner fairness for folder-sync and management deletion, 2026-09-20

Folder-sync admitted runs and provider management deletion operations now
execute on a dedicated named OS thread with a private current-thread Tokio
runtime. They no longer occupy Tokio's shared `spawn_blocking` pool while
waiting on Redis, PostgreSQL, filesystem work or the final status write. The
existing per-runtime or lifecycle admission guard remains acquired from before
input preparation through the final effect.

## Ownership and cancellation

The shared helper's parent Tokio task owns a monitor containing the worker
thread handle. A caller drop detaches the monitor, so admitted work continues
and retains its capacity. Runtime teardown drops the monitor guard, which
closes the cancellation channel and joins the worker. An in-memory async wait
therefore returns the stable runtime-stopped error; a synchronous native call
cannot be preempted and keeps the admission guard until it returns. Worker
panic, runtime-construction failure and closed result channels retain each
route's fixed task-failed mapping.

The worker's private runtime preserves timer and async-resource execution on
the owner thread. The monitor is the lifetime boundary, so the dedicated
thread cannot outlive the worker's permit or cleanup contract.

## Regression evidence

- `provider_credential_folder_sync::run` focused group: 10 passed, 0 failed.
- `http::routes::provider_deletion` focused group: 8 passed, 0 failed.
- Complete folder-sync library: 140 passed, 0 failed, 14 ignored.
- Complete Gateway lib suite after helper extraction: 3167 passed, 0 failed,
  36 ignored.
- The dedicated-owner regression runs with `max_blocking_threads = 1`, keeps a
  shared pool blocker occupied, and still observes the folder-sync operation
  start before the blocker is released. The same regression exists for the
  management deletion owner, so the shared helper's fairness contract is
  protected independently on both call sites.
- Runtime teardown tests verify async cancellation and native-call admission
  retention; caller-drop tests verify late effects and permit release.
- `cargo check --offline --locked --all-targets`: passed.
- Scoped rustfmt, effective-line tests 19/19, ratchet, and `git diff --check`:
  passed.

This closes the shared blocking-pool fairness gap for folder-sync runs and the
two management deletion routes. Watcher startup initialization, arbitrary
legacy Redis writers, production cluster-aware Redis pool/client deployment,
remote S3 success/read, concurrent filesystem replacement, Unix/macOS behavior,
and full provider/UI/Docker/release acceptance remain unverified and are not
claimed by this checkpoint.
