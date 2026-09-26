# Folder synchronization blocking run owner, 2026-09-16

The S09 blocking run owner checkpoint is verified. Admitted synchronization now
drives its complete operation on a Tokio blocking worker, keeping serial native
filesystem, hashing and JSON work off async executor workers. The original owner
failed three new scheduling regressions; the frozen candidate library passes
107/107 with six unchanged ignored tests. Real Redis runtime tests pass 15/15.
The overall optimization and release goal remains active.

## Operation and resource ownership

The existing per-application-runtime permit is acquired before preparing owned
inputs or entering the shared blocking pool. Concurrent runs still return the
stable HTTP 409/provider_credential_folder_sync_run_busy immediately. The admitted
operation can wait for a shared-pool worker while retaining the permit; another
run on that application runtime cannot enter or copy inputs during that wait.

A captured Tokio Handle drives the operation with block_on inside spawn_blocking.
One handoff covers the existing root creation, discovery, reads, JSON/hash work,
import/export, database waits and final status sequence. The same blocking closure
owns the permit through the operation. Normal callers await the full result and
monitor cleanup. Caller cancellation detaches the task handles; admitted work
continues after caller drop or auto-mode disable, retaining its capacity.

The operation occupies one blocking-pool slot even while awaiting Redis/PostgreSQL.
Admission bounds active operations per application runtime, not across the process
or separate runtimes. Shared-pool contention and throughput are not measured here.
Per-I/O offload could release slots during database waits but would introduce more
copying, scheduling and resource-lifetime boundaries; it is deferred pending evidence.

Only run/owner.rs changes production behavior in this checkpoint. The complete
run pipeline, root API, state, status CAS, import/export/deletion and containment
owners retain exact source hashes. Public call signatures, admission/preparation
prefix, stable task-failure mapping and error-kind-only logging are preserved.
No AppState or additional secret-bearing configuration is copied into the worker.

## Runtime shutdown

An unguarded Handle.block_on can make runtime Drop wait indefinitely on an
in-memory async wait. A runtime-owned monitor therefore holds a cancellation
sender while awaiting a worker-owned completion receiver. Runtime teardown drops
the monitor and wakes the worker's biased cancellation branch at an async polling
boundary. The shutdown result uses service unavailable and the fixed code
provider_credential_folder_sync_run_runtime_stopped. Worker completion, error or
panic drops the completion sender, allowing the monitor to finish.

Dropping a JoinHandle detaches its task; it does not abort it. Caller cancellation
therefore leaves both monitor and worker alive. The installed Tokio 1.51.0 source
confirms this contract, and the queued/cancelled/late-effect tests exercise it.
The candidate review's contrary claim and its dependent findings were rejected
with source evidence in reviews.md.

Native synchronous calls remain non-preemptible. Runtime teardown does not release
the permit while such a call is still executing. Shutdown can truncate async work
and does not guarantee final status persistence, transactional rollback or graceful
drain. No native deadline, total-run deadline or application shutdown protocol is
introduced by this checkpoint.

## Fresh verification

The preceding run-ownership scope, publication, accepted logs/receipts, source/test
and asset hashes, and both repository HEAD/status sets were revalidated before edits.
The baseline used unchanged production owner code plus scheduling regressions and
bounded test cleanup: 102 passed / 3 failed / 6 ignored, 3038 filtered, in 3.79 seconds.
All 108 predecessor library identities/results remained exact in that baseline.

The three failed contracts cover current-thread executor responsiveness during a
bounded native wait, Tokio timer driving on a separate stable worker thread, and
admission retained while queued behind an occupied one-slot blocking pool after
caller cancellation/disable. Their exact test bytes remain unchanged in the
candidate. Candidate library: 107/107, six ignored, 3038 filtered, in 0.93 seconds.
All original identities/results and the repaired three regressions are verified.

Two shutdown tests were added after baseline when the unguarded block_on risk was
identified. They are candidate-only safety tests, with no failing-before claim:
runtime teardown wakes an in-memory async wait without a late effect; a native
call retains admission until it returns. Bounded OS waits, a rescue sender and
joined helper threads prevent a failing shutdown test from hanging the suite.

Existing cancellation tests now wait for actual admission release before checking
resource counts or cleaning fixtures. A completion signal can precede resource
release on another thread. All existing assertions remain; the real-Redis fixture
uses a bounded subsequent manual request with PgPool=None to drain the operation
and requires the expected 503 before cleanup. Baseline-executed tests stay frozen.

Real Redis baseline: 15/15 in 0.73 seconds. Candidate: 15/15 in 1.66 seconds, zero
ignored. All 15 predecessor identities/results remain exact. Each phase used a
dedicated guarded redis:7-alpine container on a random loopback port. Both removed
only their owned container, preserved all 45 prior container identities/states,
and left zero run-owned temporary directories.

Serialized fresh gates passed:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_runtime -- --ignored --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <5 scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

Deletion integration passes 9/9 in 0.69 seconds. Checker tests pass 19/19. Both
repositories' staged/unstaged git diff --check and terminal native-idle guards pass.
Warning sets match the predecessor: three library warnings, one lib-test warning,
and the prebuilt-web-assets notice. No candidate correction after the frozen
snapshot, successful native-gate replay or proof-script correction was needed.

## Size and evidence

| Scoped file | Before | After |
| --- | ---: | ---: |
| src/provider_credential_folder_sync/run/owner.rs | 33 | 54 |
| src/provider_credential_folder_sync/run/tests.rs | 96 | 107 |
| src/provider_credential_folder_sync/run/tests/blocking.rs | new | 86 |
| tests/provider_credential_folder_sync_runtime/run.rs | 136 | 146 |
| src/provider_credential_folder_sync/run/owner/tests.rs | new | 97 |

All 61 folder-sync Rust files remain at most 500 effective lines, maximum 460.
Strict retains expected exit 1: 2163 scanned / 11 hard / 20 mandatory / 39 soft.
All 31 above-700 entries retain exact hashes/counts; clearance remains
114/145 (78.6%). Source/test union is 1868; unchanged neighbors 1863; unchanged
web/Tauri assets 22. No exception or checker-policy change is needed.

Five read-only reviews and coordinator source review cover scheduling, admission,
task/permit lifetime, cleanup, input/error safety and the test-evidence distinction.
The adopted and rejected findings, test limitations and design tradeoffs are
recorded in design.md and reviews.md. Two further read-only locators identify
watcher startup and management deletion as independent remaining blocking paths.

Evidence: target/effective-line-evidence/20260916-folder-sync-blocking-owner/.
scope.json observedAt: 2026-09-16T02:07:05.487Z. Scope SHA-256:
2be339b75d3604f4e83ca807e4dfeb27c2dd7075045f61bb6539a55fb227583d.
Snapshots, original source, logs, receipts, guards, scripts and reviews are hash-bound.
publication.json verifies the five checkpoint documents and exact repository deltas.

After publication, Gateway retains 194 modified, one unstaged deletion, two staged
deletions and 2405 untracked entries. This checkpoint adds two test files and two
documents. Neuro retains 10 modified and 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit, dependency, baseline,
exception, checker, persistent-service or release changes occurred.

## Remaining work

Watcher startup still calls synchronous mkdir and native/poll watcher construction
in watcher/runtime.rs. Watcher handle Drop also needs lifecycle review before
offloading initialization. Management deletion performs synchronous containment
checks/remove_file from two async HTTP handlers; its authorization, delete-before-DB
ordering and cancellation ownership need a separate bounded change.

The real Redis fixture has PgPool=None and proves early filesystem/error/admission
behavior. Successful PostgreSQL import/export and final status under DB load remain
unverified. Shared-pool fairness, explicit-path/DB hydration/parsed-memory budgets,
native deadlines, graceful shutdown/status priority, queued-shutdown interleavings,
concurrent path replacement/hard links, cross-process exclusion, Unix execution,
provider/UI/Docker and full release acceptance remain open. The other 31 oversized
files are untouched. S06 source/cursor/final native build remains reserved under
GWP-20260912-01; target 4200 and Neuro/release/Gateway remain unchanged.
