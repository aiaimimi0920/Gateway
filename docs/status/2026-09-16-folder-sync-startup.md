# Folder synchronization startup activation, 2026-09-16

S09 disabled-at-start activation is verified. A configured folder-sync task now
waits for enablement and starts without restarting Gateway. Runtime regressions
advance from 2 passed / 4 failed to 6/6; the existing library suite remains 56/56.
Full optimization, database synchronization and release acceptance remain open.

## Cause and change

runtime::spawn_background_tasks spawns the folder-sync task once. Management
set_runtime_enabled persists the override and updates a Tokio watch sender; it
does not respawn a task. Previously, disabled startup wrote status and returned
before subscribing, leaving later management enablement without a consumer.

watcher/runtime.rs now subscribes before the first asynchronous status operation.
After recording disabled status, it reads the latest boolean and awaits changes
until enabled. It then enters the existing directory, watcher and initial-sync
startup. The same receiver continues into the existing runtime select loop,
preserving notifications received during startup I/O. Boolean values are copied;
no watch::Ref is held across await. The wait adds no polling timer or extra task.

The unconfigured-root guard still returns before subscription or filesystem work.
Disabled startup creates no sync root or watcher. Enabled-at-start initialization,
native/poll composition, event filters, debounce, deletion retention, runtime
toggles and periodic synchronization retain their existing code. Full-source
reconstruction verifies only the early subscription, wait, log and removed late
subscription differ. No supervisor, restart registry or management API change
was needed. Independent design and candidate reviews found no introduced defect.

## Executed verification

The new integration target uses a private Redis container, a checked run-id guard,
and uniquely owned temporary directories. The interval is 3600 seconds, so a
three-second activation assertion cannot pass through periodic fallback. The
runtime command, including its environment, is recorded in the gate receipts:

    cargo test --offline --locked --test provider_credential_folder_sync_runtime -- --ignored --test-threads=1

The initial baseline attempt stopped at E0277 before executing assertions: Redis
0.27 rejects the owned String array passed to DEL. Its log, snapshot and fixture
are retained. The sole fixture correction uses Vec for the same two keys. No
assertion changed. The accepted baseline2 and candidate use identical test and
fixture bytes: baseline2 has 2 passes / 4 failures; candidate has 6/6 passes and
zero ignored tests. The tests establish:

- A disabled task can be enabled through the management domain setter, create its
  root and record a real filesystem event with enabled/watch_running status.
- The same activation works with watchers disabled and persists enabled status.
- Enablement during a blocked disabled-status write is retained. The test holds
  the sole Redis connection and polls startup to Pending before changing the flag.
- Cancelling the disabled task joins it, releases its AppState reference and
  removes the owned directory. Enabled startup and unconfigured-root exits retain
  their existing behavior.

The fixture task owns its directory through cancellation; panic cleanup also
aborts the task. All three attempted runs removed their private Redis container
and temporary directories. The same 45 pre-existing container identities and
states were preserved. The candidate's terminal guard observed an unrelated
Beaver compilation after all six tests passed. After that process finished,
cleanup and source hashes were revalidated without rerunning the successful tests.
The earlier baseline-format guard interruption is also retained.

Fresh closing gates pass:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo check --offline --locked --all-targets

The library result is 56/56, with the same identities as the previous accepted
checkpoint and 3038 filtered out. Scoped official rustfmt --check, checker tests
19/19, the effective-line ratchet and both staged/unstaged Git checks in Gateway
and Neuro pass. Strict exits 1 for unchanged legacy debt. No candidate source or
executed-test correction was required. Existing Gemini warnings remain.

## Source, evidence and repository state

Effective lines: runtime 257 -> 265; integration tests 96; fixture 178. All three
owners stay below 500. The folder-sync root remains byte-identical at 5553.
Strict scans 2112 files: 12 hard, 20 mandatory, 39 soft. All 32 files above 700
retain their prior counts and hashes; clearance remains 113/145 (77.9%).

Evidence: target/effective-line-evidence/20260916-folder-sync-startup/.
The final input union contains 1817 files, including 1814 unchanged neighbors;
22 web/Tauri assets remain unchanged. Source, logs, receipts, cleanup observations,
guards, failed preparation, source reconstruction and scripts are hash-bound.
scope.json observedAt: 2026-09-15T18:25:38.358Z.
Scope SHA-256: 70273d7287e4e7287658a3a0efbf8f65960947dd49bc727653368b047016aa02.

- runtime.rs: b32d6d11c292efaf04131127bffa287161eb52c1979f709e5c7305f55909d357.
- Integration tests: 1463c8581ec1cc5be36829e814f0871819c2e457dd009b179ae987a9d5e3c077.
- Fixture: 4f82b4cea56c3b7baf2a891b8c445b0ef54da636674c33f04b310cc3c66163cb.

publication.json verifies the final five docs, evidence, script limits and exact
Git deltas. Expected final Gateway status: 193 modified, 1 unstaged deletion,
2 staged deletions, 2332 untracked. Neuro stays 10 modified, 182 untracked.
Gateway HEAD stays 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d; Neuro HEAD stays
bf818f0324024634bc890585efb78cc8e603d11a. UTF-8 without BOM is checked. No dependency,
policy, baseline, exception, profile, persistent service, staging, commit or release
change occurred; Docker activity was limited to isolated test fixtures.

## Remaining requirements

The integration fixture has no PostgreSQL pool. It proves activation, event/status
delivery and the tested task cleanup; successful credential import/export remains
unverified. Native and poll watchers are combined, so the event source and complete
backend-thread shutdown are not separately accepted.

Queued debounce signals across disable/re-enable, rapid-toggle generations, status
read-modify-write races and startup I/O deadlines remain lifecycle work. Distinct
deletion paths still require reliable overflow/storage for a hard memory bound.
Concurrent-directory deletion races, explicit/missing DB deletion, remaining root
migration and full strict/feature/language/provider/runtime/UI/Docker/release gates
remain open. S06 original source/cursor/final build stays reserved under
GWP-20260912-01. Persistent service target stays 4200; the only release root remains
Neuro/release/Gateway. The overall optimization goal remains active.
