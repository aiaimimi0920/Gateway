# Folder synchronization enable operation ownership, 2026-09-16

S09 management enable/disable operations now share per-runtime admission and retain
their admitted work after caller cancellation. The staged inline baseline has
9 passes / 1 failure; the frozen owned-task candidate passes 10/10 in real Redis.
The unchanged library suite passes 63/63. Full optimization and release acceptance
remain open.

## Cause and implementation

Management setters previously persisted the enabled override, updated process
memory and committed status independently within each request future. Concurrent
requests could apply those stages in different orders. Dropping a request also
cancelled its remaining stages, potentially leaving an already-started update
unfinished. Status CAS alone cannot serialize the separate override/memory stages.

ProviderCredentialFolderSyncRuntime now owns a clone-shared Tokio mutex. A setter
acquires an owned permit before creating its folder configuration payload or work
task. It then spawns one operation containing the existing override -> runtime
notification -> status CAS sequence. The task retains the permit through completion
or error. A cancelled caller drops its JoinHandle while admitted work continues;
a caller cancelled while waiting for admission never spawns work.

Normal callers receive the committed status or the original typed operation error.
Failure logging occurs within owned work and prints only ErrorKind, so failures
remain observable after caller cancellation without logging Redis messages, paths,
credentials or configuration. Join failures map to a fixed server-error message.

Only the pool handle, runtime handle, owned permit and eight folder configuration
fields enter the task. Config and AppState are not cloned. The configuration
projection preserves the original root string, trimmed availability check, optional
interval/debounce, watch-running clearing and import/export/delete fields. Regular
read/watch updates consume their projection; CAS retries clone only folder fields.
The Redis override writer, status schema, public API paths and status CAS are intact.

This adds task scheduling and management serialization. It makes no latency or
throughput improvement claim. One work permit is held per runtime; waiting HTTP
requests and completed JoinHandles are not globally bounded by this change.

## Executed verification

The baseline is staged, with the mutex, owned projection, new owner and tests
already present; admitted work still executes inline. It is not an untouched-source
baseline. The candidate changes only the spawn/JoinError boundary in enable.rs.
All other source, fixture and test bytes remain identical between executions.

Both guarded real Redis runs executed:

    cargo test --offline --locked --test provider_credential_folder_sync_runtime -- --ignored --test-threads=1

Baseline: 9 passed / 1 failed. Its sole failure is the three-second timeout waiting
for an admitted enable after dropping its caller. Candidate: 10/10, zero ignored,
finished in 0.53 seconds. A held sole Redis connection and explicit first-poll order
establish admission before any persistence can proceed; no production hook or mock
Redis is used. This tests cancellation before Redis work starts, with ownership
through later awaits checked in the source review.

The other three new cases preserve cancelled-waiter behavior, final true/false
admission ordering, and permit release across two malformed-status errors followed
by repair. They already pass in the staged serialized baseline; no pre-fix ordering
failure is claimed. All six original startup/runtime test identities and bodies
remain unchanged, including real filesystem-event/status and cancellation cleanup.
The fixture still has no PostgreSQL pool, so these runs do not prove successful
credential import/export.

Fresh closing gates pass:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo check --offline --locked --all-targets

Library: 63 passed, zero failed, six ignored, 3038 filtered out. All 63 identities
are unchanged. The six unchanged Redis CAS tests remain ignored in this run; their
explicit 6/6 acceptance belongs to the preceding status-CAS checkpoint. They were
not replayed here. Scoped official rustfmt --check, checker tests 19/19, ratchet,
whole-source reconstruction and staged/unstaged Git checks in Gateway and Neuro
pass. Strict exits 1 for unchanged legacy debt. Existing Gemini warnings remain.
No candidate or fixture correction and no successful native-test replay occurred.
The first document-publication attempt stopped at the native-process guard after
another build appeared. publication-1.json retains that observation. A fresh process
check found no native build processes before publication resumed; no process was
killed and no successful gate was rerun.

Both private Redis containers are removed and all run-owned temporary directories
are gone. All 45 pre-existing container identities/states remain. Sources, tests and
22 web/Tauri assets stayed frozen throughout the serial native gates.

## Scope and evidence

Effective lines: state 134 -> 141; status 190 -> 140; runtime target 96 -> 98;
new configuration owner 44, enable owner 60, control tests 134. All changed/new
owners remain below 500. The legacy folder-sync root is unchanged at 5543.
Full source reconstruction proves state changes are limited to admission, the
status DTO/read/watch code is retained, the override writer is exact, all original
runtime tests remain, and the paired candidate changes only task ownership.

Strict: 2120 scanned / 12 hard / 20 mandatory / 39 soft. All 32 files above 700
retain their preceding hashes and counts. Clearance remains 113/145 (77.9%).
The source union is 1825 files with 1819 unchanged neighbors and 22 unchanged
assets. Dependencies, policies, baselines and exceptions are unchanged.

Evidence: target/effective-line-evidence/20260916-folder-sync-enable-ownership/.
Snapshots, original/paired sources, logs, cleanup receipts, reviews and scripts are
hash-bound. scope.json observedAt: 2026-09-15T20:46:45.271Z. Scope SHA-256:
54e65d9a03e8a1d9c4e2762c048da01a07b443e474f9c0c078983cde2feececc.

publication.json verifies the final five documents and exact Git deltas. Gateway
retains 194 modified, 1 unstaged deletion, 2 staged deletions and 2346 untracked
entries; Neuro retains 10 modified and 182 untracked. Gateway HEAD remains
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d; Neuro HEAD remains
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit, dependency, profile,
persistent service or release change occurred. Edited files are UTF-8 without BOM.

## Remaining requirements

The gate coordinates setters sharing one runtime. Direct runtime.set_enabled
calls, other processes and independent watcher/config writers are outside it.
Shared-field priority and rapidly coalesced boolean transitions remain unchanged.
Queued filesystem events across disable, debounce epochs, all-operation I/O
deadlines and full native/poll backend shutdown need further work.

Status failure still leaves earlier override/memory changes in place; the new
regression explicitly protects that existing partial-effect behavior. Ambiguous
network errors, a stuck Redis await and process/runtime termination remain limits.
There is no distributed transaction, rollback or durable operation queue. A stuck
admitted operation retains its permit until it finishes or the runtime is torn down.

Database import/export, durable audit, directory replacement races, remaining root
migration and full provider/runtime/UI/Docker/release gates remain open. S06 source,
cursor and final build stay reserved under GWP-20260912-01. Persistent target stays
4200; release root stays Neuro/release/Gateway. The overall optimization goal remains
active.
