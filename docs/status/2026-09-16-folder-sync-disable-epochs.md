# Folder synchronization disable epochs, 2026-09-16

S09 now invalidates queued and task-owned watcher work across coalesced disable /
re-enable transitions. The executable staged baseline has 66 passes / 6 failures;
the frozen candidate passes 72/72, preserving all original 63 library identities.
The unchanged guarded Redis runtime target passes 10/10. Full optimization and
release acceptance remain open.

## Failure and implementation

A boolean watch can expose only the final true value after false -> true occurs
before the consumer runs. The old consumer would skip its disable branch, retaining
an elapsed debounce timer and task-owned deleted paths. Mailbox signals queued
before disable could also be processed after re-enable or merged with new paths.
Blindly draining the mailbox when a delayed control branch runs could discard
events already admitted after re-enable.

The runtime now publishes one typed snapshot containing enabled and an owned Arc
epoch identity. set_enabled uses send_if_modified: it checks the current value,
rotates the identity on true -> false and updates enabled within the same watch
write. False -> true retains the replacement identity; repeated values neither
rotate nor notify. Old snapshots retain their Arc, preventing identity reuse while
old work is observable, without an overflowing counter or a second channel.

Mailbox send/receive takes its mutex before obtaining an owned runtime snapshot.
It removes obsolete or disabled pending nodes under that mutex before coalescing;
large discarded path sets are freed after unlocking. An open disabled receiver
discards callback sends successfully, while a closed receiver still rejects them.
The queue retains two signal kinds, exact current-epoch deletion-path union and
latest-kind error/clear ordering. A dequeued envelope carries its snapshot and is
checked again before its paths enter runtime-owned work.

PendingWatchWork owns the timer, exact pending paths and their epoch. Each control,
periodic, debounce and signal branch reconciles a fresh snapshot before admitting
work. A changed epoch or disabled state cancels the timer and clears obsolete paths.
An already-selected obsolete debounce readiness is skipped. Periodic and enable
branches can still run fresh synchronization with obsolete paths removed. A later
control notification does not erase paths already admitted in the same new epoch.

The public Rust subscribe() return changes from watch::Receiver<bool> to
watch::Receiver<ProviderCredentialFolderSyncSnapshot>. Its sole existing production
caller is migrated; the public snapshot exposes enabled(). Existing enabled() and
set_enabled() boolean contracts, HTTP status JSON, Redis keys and management
operation ownership remain. Out-of-tree Rust subscribers would need to adapt to
the typed payload; no dual-channel compatibility layer was added.

## Executed verification

The first baseline did not compile because Tokio test-util is not enabled, making
start_paused/advance unavailable. That snapshot and all ten compile errors remain
in the evidence directory; no assertions ran. Only work/tests.rs was corrected to
use the existing actual-timer conventions, without changing dependencies/features.
ZERO timer readiness is awaited under a one-second timeout; cancelled/hour-long
timers are polled. No arbitrary delay determines the toggle sequence.

baseline2 contains all snapshot/envelope/work-owner plumbing but never rotates the
epoch. It is explicitly a staged baseline, not untouched source. Candidate changes
only the identity rotation in state.rs; all other source and test bytes stay exact.

    cargo test --offline --locked --lib folder_sync -- --test-threads=1

baseline2: 66 passed / 6 failed / 6 ignored. Candidate: 72/72, zero failed, six
ignored, 3038 filtered out, finished in 0.19 seconds. The six repaired failures
cover obsolete queue entries, cross-epoch path merge, dequeued envelopes, four
parallel senders across the boundary, selected elapsed timers, and late control
reconciliation. Three new preservation cases cover disabled storms/closed sends,
same-value enablement/first deadlines, and observed/repeated disablement.

Four barrier-coordinated senders preserve all 256 new paths and discard four old
ones. The original 400-distinct-path and 10,000-duplicate-event tests remain intact.
All eight original mailbox test bodies are byte-preserved through a test-only
adapter that projects envelope.signal. All 63 original library identities remain.
The six unchanged Redis CAS tests are ignored in this run and retain their earlier
explicit acceptance; they were not replayed here.

Fresh closing gates pass:

    cargo check --offline --locked --all-targets
    cargo test --offline --locked --test provider_credential_folder_sync_runtime -- --ignored --test-threads=1

The unchanged Redis target passes 10/10, zero ignored, in 0.47 seconds. It retains
real filesystem-event/status, startup, caller-cancellation and task cleanup checks.
The fixture has no PostgreSQL pool, so it does not prove successful database
credential synchronization. The new epoch interleavings use real runtime snapshots,
mailbox and timer owners in library tests, not OS-event occurrence timestamps.

Scoped official rustfmt --check, checker tests 19/19, ratchet, source-boundary checks
and both repositories' staged/unstaged Git checks pass. Strict exits 1 for unchanged
legacy debt. Existing Gemini warnings remain. After compile/formatter success, a
native-process guard stopped checker admission. The retained checker-tests-1.json
records it; a fresh observation found no native processes. Only unfinished gates
resumed, with completed receipts/hash checks retained. No process was killed, no
candidate source correction occurred and no successful native gate was replayed.

The private Redis container and all run-owned temporary directories are removed.
All 45 pre-existing container identities/states remain. Native gates were serial
against frozen source/tests and 22 web/Tauri assets.

## Scope and evidence

Effective lines: state 141 -> 165; watcher 162 -> 163; runtime 255 -> 256; mailbox
119 -> 161; original mailbox tests 134 -> 136; new work owner 28, work tests 93,
test adapter 16, epoch tests 127. All changed/new owners remain below 500. The
legacy folder-sync root remains unchanged at 5543.

Source checks retain non-folder state, complete backend/callback code except module
wiring, startup I/O/initial run, mailbox coalescing/drop semantics and original test
bodies. They verify four branch reconciliation sites, stale timer/envelope checks,
the sole initial test-fixture correction and the exact candidate-only rotation.

Strict: 2124 scanned / 12 hard / 20 mandatory / 39 soft. All 32 files above 700
retain identical hashes/counts. Clearance remains 113/145 (77.9%). Source union
1829, unchanged neighbors 1820, unchanged assets 22. Policies, baselines, exceptions
and dependency files are unchanged.

Evidence: target/effective-line-evidence/20260916-folder-sync-disable-epochs/.
scope.json observedAt: 2026-09-15T21:33:15.006Z. Scope SHA-256:
a41c82824699bf8f0354684573e0a5e8dc99ae0c411039506fcb24ad07f879bd.
Logs, original/paired sources, corrections, guards, reviews and scripts are hash-bound.

publication.json verifies all five final documents and exact Git deltas. Gateway
retains 194 modified, 1 unstaged deletion, 2 staged deletions and 2352 untracked
entries; Neuro retains 10 modified and 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a respectively. Edited files are UTF-8 without
BOM. No staging, commit, profile, persistent service or release change occurred.

## Remaining requirements

The boundary is mailbox snapshot admission and branch-local run admission. A state
change after a branch snapshot does not roll back work already admitted by it.
Native OS events first delivered after re-enable receive the new epoch; their
historical occurrence time cannot be inferred here. No backend callback or running
database operation is forcibly cancelled. Obsolete memory is released when the
consumer or a callback next reconciles, rather than synchronously by the toggle.

Current-epoch distinct path bytes and external snapshot holders are not globally
bounded. The change adds snapshot Arc operations and one small allocation per
true-to-false transition; no latency/throughput improvement is claimed. Initial
sync/status I/O deadlines, shared status-field ordering, durable audit, full backend
shutdown, database synchronization and directory-replacement races remain open.
Remaining root migration and full provider/runtime/UI/Docker/release gates remain.
S06 source/cursor/final build stays reserved under GWP-20260912-01. Persistent target
stays 4200; release root stays Neuro/release/Gateway. The overall goal remains active.
