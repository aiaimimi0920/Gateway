# Folder-sync watcher deletion-intent bounds, 2026-09-18

This incremental checkpoint is verified. Database source-path retention is bounded
at import time, and watcher-side deletion intent now has an independent finite
budget with fail-closed overflow handling. The overall optimization and release
goal remains active.

## Implemented boundary

`BoundedPathSet` stores normalized deleted relative paths in a `HashSet<String>`
and charges each string's allocated capacity plus a fixed 128-byte entry margin.
The 8 MiB cap uses
saturating arithmetic. Duplicate paths do not increase the charge. Event creation,
mailbox coalescing and pending work merge all use the same owner.

An overflow clears all retained paths and sets an internal marker. The watcher
runtime cancels its debounce, clears the marker, writes
`provider credential folder watcher deletion intent exceeded its byte limit; explicit deletion was skipped`
to `last_watch_error`, and does not run deletion for the partial event. A later
event starts from an empty set. Disable or epoch reconciliation also clears the
marker and paths. The existing status payload shape and two-node mailbox contract
are unchanged.

## Verification

Fresh final-source gates:

    cargo test --offline --locked --lib provider_credential_folder_sync:: -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --check <nine scoped watcher source/test files>
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts
    git diff --check
    git diff --cached --check

The folder-sync library result is 126 passed, zero failed and 12 ignored. New
tests cover duplicate accounting, per-event overflow, overflow propagation during
mailbox merge, runtime-work recovery and epoch cleanup. All-targets passes with
the existing Gemini warnings; checker tests pass 19/19 and the ratchet passes.
The guarded PostgreSQL/Redis suite was not rerun in this checkpoint because the
current environment does not expose its required `GATEWAY_DELETE_TEST_RUN_ID`
and guarded database URLs; the prior retained-path source snapshot passed 13/13.

## Size and scope

| File | Effective lines |
| --- | ---: |
| `src/provider_credential_folder_sync/path_budget.rs` | 129 physical, below 250 effective target |
| `src/provider_credential_folder_sync/watcher.rs` | existing owner, bounded wiring only |
| `src/provider_credential_folder_sync/watcher/mailbox.rs` | existing owner, bounded merge wiring |
| `src/provider_credential_folder_sync/watcher/runtime.rs` | existing owner, fail-closed status wiring |
| `src/provider_credential_folder_sync/watcher/work.rs` | existing owner, bounded work wiring |

No dependency, policy, baseline, exception, staging, commit, release or persistent
service state changed. Existing dirty Gateway and Neuro worktrees are preserved.

## Remaining work

The cap begins after `notify` has created an event, so native backend allocation
and event path vectors remain outside this owner. Database and material bounds,
native operation deadlines, complete shutdown drain, status races, blocking-pool
fairness, concurrent replacement and hard-link/reparse races, Unix/macOS behavior,
selected remote object-storage success/read limits and complete provider/UI/Docker/
release acceptance remain open. The single-account transient hydration bound is now
covered by the 2026-09-18 checkpoint. Port 4200 and `Neuro/release/Gateway` were
not touched.

Evidence: `target/effective-line-evidence/20260918-folder-sync-watcher-path-budget/`.
[Lane](../plan/parallel-lanes/folder-sync-watcher-path-budget.md).
