# Folder synchronization watcher signals and timer cleanup, 2026-09-16

S09 signal coalescing and pending-timer cancellation are verified. The event queue
now has at most three pending nodes, preserving every distinct deletion path.
Distinct-path memory, remaining lifecycle cases and full Gateway acceptance stay open.

## Implemented changes

The watcher callbacks and runtime now share a private single-consumer mailbox.
It retains one node for each exhaustive signal kind: FilesystemEvent, DebouncedSync,
and WatcherError. A repeated filesystem signal unions its exact deleted-path set
with the previous pending set. It never invents a directory ancestor or drops an
explicit deletion because the queue is full. Repeated diagnostics keep the latest
message. Replacing a kind moves it to the tail, retaining latest-kind order and the
final error/clear relationship used by watch-status persistence.

This is an explicit coalescing policy: intermediate event-status writes and error
log messages can disappear, and merged paths can reach a later fixed-window sync.
The retained path data still schedules processing. Native and poll backends, filter
rules, successful-sync clearing, failed-sync retention and delete_missing policy
remain unchanged. In particular, periodic scanning with delete_missing=false cannot
repair a lost explicit deletion; the implementation therefore retains distinct paths.

One mutex protects queue mutation and sender/receiver lifecycle. Callback send does
not wait for capacity. Mutex contention and path merging remain synchronous and
scale with incoming data. Receiver Drop closes future sends, takes the pending queue
under the lock and frees potentially large path sets outside the lock. Sender clones
and drops update a shared count, and the final sender wakes the waiting receiver.
No lock is held across await. recv requires mutable access to its sole receiver.

The separate DebounceTask retains the exact original spawn/sleep/send operation and
explicit abort method. Its Drop now aborts pending timer work when the parent future
is cancelled. The existing first-event scheduling boolean and delay remain. An
already emitted DebouncedSync is not retracted; this is not a timer-generation fix.

A suggested single-receiver lost-wakeup concern was checked against locked Tokio
1.51.0's own Notify documentation: notify_one saves a permit, and its official MPSC
example checks the queue before notified().await. The enable requirement applies
to concurrent multi-consumer recv. The implemented pattern matches the former.

## Fresh regression and preservation evidence

Frozen baseline adapters retained direct unbounded mpsc and the bare detached
JoinHandle. Both baseline and candidate ran GATEWAY_PREBUILT_WEB_UI=1 with:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1

Baseline: 51 passed / 5 failed. First frozen candidate: 56/56, 3038 filtered out.
All 45 original test identities and assertions remain unchanged; eleven new tests
stay byte-identical between runs. Four signal-policy groups and one parent-cancellation
group fail before and pass after; six new preservation groups pass in both runs.

The tests demonstrate:

- 10,000 duplicate filesystem signals occupy one pending node and retain the path.
- Error bursts retain one latest diagnostic; mixed bursts occupy at most three nodes.
- Error/change order preserves the final clear state and both exact deletion paths.
- Four concurrent senders preserve all 400 distinct deletion paths without adding
  ancestor paths. Receiver drop, final-sender drop and cancelled recv keep closure
  and notification behavior correct.
- Cancelling a real parent task releases the pending timer's channel sender within
  the timeout. Explicit abort still releases it, and a normal timer emits once then
  closes. Tests use task/oneshot coordination without paused-clock feature changes.

The verifier reconstructs the complete original watcher/runtime projections with
only imports, sender/channel aliases and the extracted timer call changed. Baseline
mailbox and timer adapters are checked exactly. Candidate wiring and both test files
remain identical to baseline, while timer code differs only by its Drop guard.
All other source and existing test bytes retain their hashes. Independent candidate
review found no introduced concurrency, closure or distinct-deletion-loss defect.

All-targets cargo check, scoped official rustfmt --check, checker tests 19/19,
ratchet and both staged/unstaged Gateway and Neuro Git checks pass. Native phases
are serialized and terminal; baseline/candidate warning sets match. The inherited
Gemini unused HashMap warning remains. No failed candidate, executed-test rewrite,
formatter failure or source-proof correction occurred in this lane.

Effective lines: watcher 161 -> 163; runtime 257 unchanged; mailbox 133; debounce 22;
mailbox tests 135; debounce tests 51. All changed/new owners are below 500. Root stays
byte-identical at 5553. Strict remains exit 1: 2110 scanned / 12 hard / 20 mandatory /
39 soft; all 32 above-700 files are unchanged. Clearance remains 113/145 (77.9%).

## Evidence and repository state

Evidence: target/effective-line-evidence/20260916-folder-sync-watcher-signals/.
Before capture revalidated the previous watcher scope/publication, 1811 inputs,
22 assets, five docs and exact Git states. Final union is 1815 inputs, with 1809
unchanged neighbors and 22 unchanged web/Tauri assets. Root, deletion/status,
static containment, previous tests and shared fixtures remain byte-identical.

scope.json observedAt: 2026-09-15T17:12:40.936Z.
Scope SHA-256: b0d6433a7f0079785c193d1f7c45f48c92ab1a8a6d3652e92c974f97862b3400.
Source SHA-256 values:

- watcher.rs: 8a1c3406daeae13e4e73cb4156304dffd7f6ec22c83f61651c19e73028859903.
- watcher/runtime.rs: d884093fbeac21f2bf1f7264ffeb94cf9a340ecf282fe20a7cf3aa87da780431.
- watcher/mailbox.rs: 841cb6a7201e332781d64b5d47c37f80610f217b807ca0d140eaba374d604030.
- watcher/debounce.rs: 36fa2bc14271d6c44834ecf99d3b32d53c666ec60fa95945856a675e30bb79a0.
- mailbox tests: 5e574cf475cb120d2004d1407a16a278608660f53f5df722268b6e973afa3407.
- debounce tests: 6148368970276344abc44f8e756e694930fae834a0fdcbec2cdb67d0c9e5422c.

Snapshots, source, logs, receipts, guards, inventory and scripts are hash-bound.
publication.json verifies final five docs, evidence, script limits and exact Git
deltas. Gateway final expected status: 193 modified, 1 unstaged deletion, 2 staged
deletions, 2328 untracked. Neuro remains 10 modified, 182 untracked. HEADs stay
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No dependency, checker, baseline, exception,
profile, service, staging, commit or release change occurred. UTF-8 without BOM is checked.

## Remaining requirements

Three queue nodes do not impose a hard total-memory bound. The mailbox and runtime
pending set retain all distinct deleted paths; extreme distinct-path growth requires
a reliable overflow/storage design. The current periodic rescan is not a safe
replacement for that deletion intent, and ancestor compaction can broaden deletion.

Next lifecycle work should cover disabled-at-start followed by enable, queued timer
signals across disable/re-enable, and native backend shutdown. Live Redis/DB/status
and provider validation remain necessary. Concurrent-directory replacement in file
deletion and shared-snapshot explicit/missing DB deletion still need separate proof.

Root migration and strict/feature/language/provider/runtime/UI/Docker/release gates
remain open. S06 original source/cursor and final-build coordination remain reserved
under GWP-20260912-01. Persistent service target stays 4200; the only release root is
Neuro/release/Gateway. Full optimization remains active.

