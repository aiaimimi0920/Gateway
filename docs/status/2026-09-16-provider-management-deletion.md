# Provider management deletion ownership, 2026-09-16

The management-deletion checkpoint is verified. Credential and account DELETE
operations now run their complete file, database and Redis sequence on an owned
blocking worker. Cancelling the HTTP caller leaves the admitted operation running.
The focused owner suite passes 7/7 and real PostgreSQL/Redis route contracts pass
6/6. The overall optimization and release goal remains active.

## Ownership and API behavior

GatewayLifecycleState owns one independent management-deletion permit per AppState.
Both DELETE routes share it. Management authorization and required PostgreSQL
validation run before admission; admission precedes Arc/pool copies and database
hydration. A concurrent deletion returns HTTP 409 with
`provider_management_delete_busy`. There is no waiting deletion queue. This is a
new overload policy; callers issuing parallel deletions must handle the conflict.
Folder synchronization and enable updates retain their separate admission rules.

The worker owns the original complete ordered operation. Credential deletion reads
the record, conditionally deletes its folder-sync file, deletes its DB/object
payload, clears Redis runtime keys and builds the existing response. Account
deletion retains the original serial credential loop, then account DB deletion,
payload-cache deletion and account runtime cleanup. File errors still stop before
DB mutation. Missing files remain harmless; disabled/manual credentials retain
their files. Only the credential route suppresses a concurrent DB-deletion 404.

Complete formatted-source comparison preserves both original handler bodies with
only the owner wrapper and borrowed pool arguments added. Authorization, localized
messages, response fields, error handling, ordering and unrelated functions remain
exact. The public synchronous filesystem helper is unchanged and now executes on
the blocking worker when invoked by these two management routes. Its enabled,
path-validation, root-resolution and containment order remains unchanged.

The admitted future captures an AppState Arc, a PgPool clone and the existing owned
ID. This deliberately retains live folder enable/config state without copying
headers, management tokens or hydrated credential payloads. The blocking worker
holds admission during pool queueing, native calls and final cleanup. A Tokio
monitor wakes cancellable async waits during runtime teardown. Caller cancellation
only detaches the result; errors are logged by kind without captured request data.
Worker panic maps to the fixed `provider_management_delete_task_failed` API code.

## Fresh verification

Before edits, the previous watcher scope, publication, source/test/assets, receipt
hashes and both repository HEAD/status sets were revalidated. The library baseline
used a test-only inline owner while production routes remained unchanged: 2 passed,
5 failed, zero ignored, 3160 filtered, in 1.04 seconds. The frozen candidate passes
all seven identities in 0.03 seconds. Repaired cases cover executor responsiveness,
caller-drop cleanup/admission, rejection before preparation, queued blocking work
and panic mapping. Error propagation and runtime-teardown cleanup remain green.

The real route baseline required two fixture corrections. Initial compilation used
incorrect URL/router module paths. Baseline2 then exposed an invalid synthetic
execution mode; the actual enum requires `direct_http`. Both failed attempts are
retained. No test assertions or already executed library test files changed.
Accepted unchanged-production baseline3: 4 passed / 2 failed in 5.49 seconds.
Candidate: 6/6, zero ignored, in 1.05 seconds, with every baseline3 test byte frozen.

Each cancellation contract locks a real PostgreSQL credential row, waits for the
real file removal, aborts the actual router request and releases the lock. It then
checks eventual database deletion and all four credential runtime keys. Account
cancellation additionally checks account removal and its five cache/runtime keys.
The opposite DELETE route receives a conflict while ownership remains held;
unauthorized requests remain rejected. Success cases compare complete JSON
responses. Invalid paths preserve DB rows and Redis keys; disabled/manual cases
preserve files. Inline payload storage is used; no object-storage success is claimed.

Serialized candidate gates passed:

    cargo test --offline --locked --lib provider_deletion -- --test-threads=1
    cargo test --offline --locked --test provider_management_deletion -- --ignored --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo test --offline --locked --test internal_management_access_contract -- --test-threads=1
    cargo test --offline --locked --test internal_management_routes_contract -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <10 scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

Filesystem deletion contracts pass 9/9; management auth targets pass 4/4 each;
checker tests pass 19/19. Library and runtime warning sets match their accepted
baselines. Both repositories' staged/unstaged diff checks and native-idle guards
pass. A transient pre-baseline native guard rejection was resumed after those
processes exited. No process was killed and no successful native gate was replayed.
No source/test correction was required after candidate freeze.

Four fixture phases each used newly owned PostgreSQL and Redis containers with
random loopback ports and matching run sentinels. All eight containers and their
anonymous volumes were removed; all 45 prior container identities/states were
preserved. Successful fixtures drain worker ownership, close pools, drop their
unique schema and remove their roots. Failed-attempt DB/Redis cleanup is guaranteed
by the outer owned-container finally block; no owned temporary roots remain.

## Size, review and evidence

| Scoped file | Before | After |
| --- | ---: | ---: |
| src/state.rs | 171 | 175 |
| src/http/routes/mod.rs | 36 | 37 |
| src/http/routes/internal_provider_credentials/mutations.rs | 231 | 239 |
| src/http/routes/internal_provider_accounts/accounts.rs | 183 | 193 |
| src/http/routes/provider_deletion.rs | new | 54 |
| src/http/routes/provider_deletion/tests.rs | new | 120 |
| src/http/routes/provider_deletion/tests/lifetime.rs | new | 76 |
| tests/provider_management_deletion.rs | new | 155 |
| tests/provider_management_deletion/fixture.rs | new | 230 |
| tests/provider_management_deletion/schema.rs | new | 31 |

Every scoped file remains below 500 effective lines. Strict retains expected exit
1: 2173 scanned / 11 hard / 20 mandatory / 39 soft. All 31 above-700 entries retain
their exact hashes/counts; clearance remains 114/145 (78.6%). Source/test union is
1878, unchanged neighbors 1868, unchanged web/Tauri assets 22. No checker, baseline,
policy or exception change was needed.

Read-only discovery and independent frozen owner/route reviews were followed by
coordinator source checks. The review record retains rejected claims about a
nonexistent lookup column and missing cross-route coverage. Scoped review covered
auth, input/secret handling, native blocking, admission, cancellation, cleanup,
database/Redis ordering and fixture containment.

Evidence: `target/effective-line-evidence/20260916-provider-management-deletion/`.
Scope observedAt: 2026-09-16T03:54:04.703Z. Scope SHA-256:
`1aec7f90e2381c85681ca1398ee4224c03ff0256e5c9cdb348d652e407f9d38d`.
Original/baseline/candidate snapshots, test logs, receipts, source proof, fixture
cleanup and review records are retained. Publication verifies the five checkpoint
documents and exact repository path-status changes.

After publication, Gateway retains 194 modified, one unstaged deletion, two staged
deletions and 2419 untracked entries; Neuro retains 10 modified and 182 untracked.
HEADs remain `4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d` and
`bf818f0324024634bc890585efb78cc8e603d11a`. No staging, commits, dependency changes,
persistent-service changes or release writes were performed.

## Remaining work

One blocking-pool slot remains occupied while each admitted operation awaits async
DB/object/Redis work. Native calls cannot be preempted. Runtime teardown, process
exit, I/O errors and cross-process races can still leave partial progress; there is
no durable transaction, compensation or restart recovery. Application drain does
not yet account for detached deletions. Account-list hydration is still unbounded;
per-AppState admission does not establish a process-wide quota or synchronization
with folder runs. Combined native-call/teardown coverage and shared-pool fairness
remain open.

Successful folder import/export, DB hydration and parsed-memory/path bounds,
watcher deadlines/full drain/status priority, filesystem replacement/hard-link
races, cross-process exclusion, Unix/macOS, provider/UI/Docker and full release
acceptance remain open. The other 31 oversized files are untouched. S06 source,
cursor and final native build remain reserved under GWP-20260912-01; target 4200
and Neuro/release/Gateway remain unchanged. The overall optimization goal is active.
