# Folder-sync enabled atomic commit, 2026-09-18

This checkpoint closes the shared enabled/status write boundary and the observer
race left after the shared read projection. The public status schema and Redis key
names remain unchanged.

## Commit and observer rules

- Management enable/disable uses one Redis Lua CAS over both the status key and
  the enabled override key. It compares both raw snapshots, then writes the new
  status and override in one Redis script invocation.
- Rust still decodes and encodes status JSON, so large integer fields retain their
  existing precision behavior. A malformed status fails before either key or the
  process-local runtime changes.
- Watcher and completed-run observers read status plus enabled override together
  and CAS the same two-key snapshot. When an override exists, it is projected as
  the authoritative cross-process enabled value; when absent, the current process
  runtime remains the startup owner. A concurrent management commit forces the
  observer to reload instead of replaying stale state.
- The management status read path continues to use one MGET snapshot and does not
  mutate local runtime state.

Shutdown ownership is explicit at startup: an enabled runtime's first status
publication drains through Redis, while disabled startup and later watcher or
diagnostic writes remain cancellable. Admitted sync runs still drain through their
final status CAS.

## Verification

- Guarded Redis status-store suite: 7 passed, 0 failed.
- Guarded folder-sync runtime suite: 22 passed, 0 failed.
- Cross-runtime watcher convergence passed for cooperating Gateway runtime tasks
  through Redis Pub/Sub, including enabled-override re-read after a forced listener
  disconnect and reconnect when the override changed without a corresponding event.
- Focused status tests: 9 passed, 7 ignored.
- Complete folder-sync library: 138 passed, 0 failed, 13 ignored.
- `cargo check --offline --locked --all-targets`: passed with existing Gemini
  warnings.
- Effective-lines tests: 19 passed, 0 failed; ratchet: passed for 2,185 files
  (`>1500=11`, `701-1500=20`, `501-700=39`).
- Scoped rustfmt and `git diff --check`: passed.

The guarded fixture used a newly created Redis container and a unique run guard;
the pre-existing Gateway, Redis, PostgreSQL and object-store containers were not
modified.

## Remaining work

This proves the two-key Redis ordering, cooperating Gateway runtime watcher
convergence, and the local runtime lifecycle. It does not create a distributed
lease for arbitrary older writers, provide a product-level guarantee during a
disconnect window, prove Redis Cluster cross-slot deployment, or cover remote S3
success/read, shared blocking-pool fairness, filesystem replacement and
hard-link/reparse races, Unix/macOS behavior, or full provider/UI/Docker/release
acceptance. S06 source/cursor/final build ownership remains reserved.
