# Folder synchronization run ownership, 2026-09-16

The S09 run-admission and cancellation-ownership checkpoint is verified. All
manual, watcher and refill synchronization calls now share one per-runtime run
permit. The original implementation failed three new real-Redis regressions;
the unchanged candidate runtime tests pass 15/15. The complete folder-sync library
group passes 102/102, with six unchanged ignored tests. The overall optimization
and release goal remains active.

## Behavior and ownership

Admission uses a shared Arc<tokio::sync::Mutex<()>> with try_lock_owned. A busy
request returns immediately with HTTP status 409 and the stable code
provider_credential_folder_sync_run_busy. It does not wait for Redis or copy the
configured root, explicit deletion paths or pool handles. There is no queue of
pending synchronization operations. Runtime clones share the permit; separately
constructed runtimes remain independent. The enable-update lock stays separate.

An admitted operation owns only Redis/PgPool handles, folder-specific configuration,
runtime state, the configured root result and explicit deletion intent. No AppState
or unrelated secret-bearing configuration is cloned. A spawned async task owns the
permit through the entire operation, including final status CAS. The requesting
future awaits its complete result. Dropping that future detaches admitted work;
it does not release capacity or cancel filesystem/DB/status work partway through.
Auto-mode disable also does not cancel an already admitted run. This policy permits
late effects after caller cancellation and must be retained by later I/O offloading.

Operation errors preserve their existing result and release admission. New failure
logging includes only error kind. Task failure maps to the fixed
provider_credential_folder_sync_run_task_failed code and message; task unwinding
releases its permit. No new unsafe code, dependency or configuration option was added.

HTTP import/export, initial watcher synchronization, enable/periodic/debounce
watcher branches and credential refill all reach this owner. A busy watcher run
does not clear work.deleted_paths: existing success-only clearing remains. Existing
disable/epoch invalidation is unchanged. Manual synchronization remains available
while auto mode is disabled; phase-specific import/export configuration still applies.

## Preserved contracts

The operation retains this ordering: read/decode status; return root-resolution
errors; create root; require PostgreSQL; validate/run import then export; record
completed phases, counters, deletion history and error; return the phase result.
Root spelling is copied/resolved after admission, but resolution errors remain
deferred until after the initial status read. Existing early errors still bypass
final run-status recording. Successful phase timestamps and partial-phase counters
are unchanged. Final status projects current runtime enablement on every CAS retry,
while immutable folder configuration stays bound to the admitted operation.

The source proof compares the complete operation flow after explicit ownership
substitutions and formatting normalization, separately checks literal messages
byte-for-byte, verifies only the run gate was added to AppState runtime ownership,
and retains the exact apply_run_results body. Existing test wiring differs only
by the new run test module. Source/test neighbors and web assets remain hash-exact.

## Fresh verification

The preceding filesystem-budget scope, publication, accepted library receipt/log,
all source/test/assets and both repository HEAD/status sets were revalidated before
edits. The real Redis baseline ran against unchanged production code: 12 passed,
3 failed, zero ignored, in 5.86 seconds. Candidate runtime: 15/15, zero ignored,
in 0.67 seconds. All ten existing runtime identities/results are retained; all five
new runtime tests and the existing fixture remain unchanged across both runs.

Three regressions establish immediate busy return while Redis is held, retained
admission after caller cancellation/disable, and completion of the admitted
filesystem effect after caller cancellation. Two preservation tests cover malformed
status/root/PostgreSQL error ordering and all manual directions while disabled.

Five new library tests cover skipped input preparation while busy, runtime-clone
sharing, detached late effects/resource release, operation error/panic release,
and independent runtime/enable locks. The full library group passes 102/102, six
ignored, 3038 filtered, in 0.67 seconds. All 103 previous library identities/results
are exact. These owner tests supplement the failing-before real-Redis regressions;
no failing-before claim is made for the newly introduced private owner API.

Fresh serialized gates passed:

    cargo test --offline --locked --test provider_credential_folder_sync_runtime -- --ignored --test-threads=1
    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <10 scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

Deletion integration passes 9/9 in 0.28 seconds. Checker tests pass 19/19. Both
repositories' staged/unstaged git diff --check and the final native-idle guard pass.
Existing warnings remain three library warnings, one lib-test warning and the
prebuilt-web-assets notice; no new warning persists. No candidate source correction
or successful native-gate replay was needed.

Each real-Redis phase used its own guarded redis:7-alpine container with a random
127.0.0.1 port and unique ownership label. Both phases removed only their own
container, preserved all 45 pre-existing container identities/states, and left
zero run-owned temporary directories. No persistent Gateway service was changed.

## Size, review and evidence

| Scoped file | Before | After |
| --- | ---: | ---: |
| src/provider_credential_folder_sync.rs | 126 | 59 |
| src/state.rs | 165 | 170 |
| src/provider_credential_folder_sync/run.rs | new | 122 |
| src/provider_credential_folder_sync/run/owner.rs | new | 33 |
| src/provider_credential_folder_sync/run/tests.rs | new | 96 |
| src/provider_credential_folder_sync/status.rs | 140 | 149 |
| src/provider_credential_folder_sync/status/config.rs | 44 | 54 |
| src/provider_credential_folder_sync/status/run.rs | 53 | 52 |
| tests/provider_credential_folder_sync_runtime.rs | 98 | 100 |
| tests/provider_credential_folder_sync_runtime/run.rs | new | 136 |

All 59 folder-sync Rust files remain at most 500 effective lines, maximum 460.
No exception is needed. Strict retains expected exit 1: 2161 scanned / 11 hard /
20 mandatory / 39 soft. All 31 above-700 files retain exact hashes/counts; clearance
remains 114/145 (78.6%). Current source/test union is 1866; unchanged neighbors 1856;
unchanged web/Tauri assets 22.

Three read-only reviews examined baseline, ownership and wiring. Their comparison
and coverage limits, missed refill locator and fixture-timing corrections are
recorded in reviews.md. The evidence script's initial import-versus-call marker
failure is retained as verify-initial.mjs and corrected in the accepted verifier.
This was an evidence-processing correction after successful native gates; frozen
source/tests were not changed or rerun.

Evidence: target/effective-line-evidence/20260916-folder-sync-run-ownership/.
scope.json observedAt: 2026-09-16T01:23:50.975Z. Scope SHA-256:
b28d345ae5be74ae30b8366b781e04402eb55ecb2de8dddc7087c3fd67a62d2e.
Snapshots, original source, logs, receipts, guards, scripts and reviews are hash-bound.
publication.json verifies the five checkpoint documents and exact repository deltas.

After publication, Gateway retains 194 modified, one unstaged deletion, two staged
deletions and 2401 untracked entries, including four new source/test files and two
new documents. Neuro retains 10 modified and 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit, dependency, checker,
policy, baseline, exceptions, persistent-service or release changes occurred.

## Remaining work

Filesystem discovery, JSON work and reads/writes still run synchronously on Tokio
workers. Next work must offload those operations without releasing this run permit
while a blocking task can still mutate files. Deadlines, total run duration,
shutdown drain and status priority remain open. Admission is per runtime, not a
cross-process or shared-directory lease; management deletion remains a separate
operation. Distinct explicit-path and DB hydration/parsed-JSON budgets also remain.

The Redis runtime fixture uses PgPool=None. It proves admission and early filesystem
effects, not successful PostgreSQL import/export, late DB status persistence or
provider completeness. Real PostgreSQL success, concurrent path replacement/hard
links, Unix execution, backend watcher failure combinations, UI/Docker runtime and
full release acceptance remain open. The other 31 oversized files are untouched.
S06 source/cursor/final native build remains reserved under GWP-20260912-01; target
4200 and Neuro/release/Gateway are unchanged. The full goal stays active.
