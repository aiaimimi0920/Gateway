# Folder synchronization owned timer, 2026-09-16

S09 queued-timer cancellation is verified. Debounce is now a runtime-owned Sleep;
disable drops even elapsed readiness, with no timer message left in the mailbox.
The library suite passes 60/60 and the unchanged isolated Redis runtime suite
passes 6/6. Full optimization, database synchronization and release acceptance
remain open.

## Cause and change

The old DebounceTask sent DebouncedSync to the mailbox after its sleep. Aborting
the task on disable could not retract a message already queued. After re-enable,
that old message could run an extra sync. With a filesystem message ahead of it,
the stale message could also consume the subsequently scheduled timer window.
Pending deletion paths are owned by one runtime consumer; no concurrent mutation
of a shared HashSet is alleged.

DebounceTimer now owns Option<Pin<Box<Sleep>>>. Its first schedule creates the
sleep; later events keep that deadline. Cancelling a borrowed ready() future when
another select branch wins retains the owned sleep. Completion consumes it once.
Disable drops the owner state, including elapsed readiness. The runtime selects
the timer directly and checks the latest enabled value before running the existing
watch-triggered synchronization body. No additional task or generation protocol
is introduced. Each window no longer spawns a timer task or sends a timer message;
one pinned sleep allocation remains while a window is active.

DebouncedSync, the redundant scheduling boolean, JoinHandle abort/drop plumbing
and the obsolete test-only try_recv helper are removed. The mailbox has two
pending kinds: filesystem changes and latest watcher error. Exact deletion-path
unions and latest-kind diagnostic ordering remain. Distinct-path bytes still grow
with data size. Timer-versus-event readiness now follows runtime select scheduling;
no strict timer/mailbox FIFO or disable linearization guarantee is claimed.

Full-source reconstruction preserves the non-timer runtime code, watcher setup,
filters, status writes and mailbox behavior outside the removed timer protocol.
Startup, periodic and re-enable sync bodies are unchanged. First-event fixed-window
coalescing is retained; this does not introduce trailing-edge debounce.

## Executed verification

The pre-change diagnostic adds one expired-timer cancellation assertion to the
three existing timer tests. It fails with an already queued message: 3 passed /
1 failed. Its source, log and snapshot remain in the new evidence directory.

The removed task/channel protocol requires replacing its three tests. Seven new
tests exercise actual Tokio Sleep ownership: elapsed cancellation, a new window
after cancellation, first-event deadline retention, cancellation of the borrowed
select wait, one-shot completion, and registered-waker release on cancel/drop.
These are owner-level tests, not byte-identical paired tests or a duplicated
runtime state machine. The old failure and new owner proof have distinct artifacts.

The frozen candidate passes:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo check --offline --locked --all-targets
    cargo test --offline --locked --test provider_credential_folder_sync_runtime -- --ignored --test-threads=1

The library result is 60/60 with 3038 filtered out. All 53 non-timer identities
remain, with the mixed-storm test renamed from three nodes to two. Seven owner
tests replace three obsolete timer tests. The six unchanged runtime tests pass
with zero ignored during the explicit isolated Redis run. They retain disabled
startup activation, actual filesystem-event/status observation, activation without
watchers, enable-during-status-write notification, cancellation and root guards.
They do not prove deterministic enable-versus-timer business-branch ordering.

Scoped official rustfmt --check, checker tests 19/19, ratchet and staged/unstaged
Git diff --check in Gateway and Neuro pass. Strict exits 1 for unchanged debt.
All native gates were serialized with frozen source/assets, and completed without
candidate correction or successful-test replay. Existing Gemini warnings remain.
Independent production and test reviews are recorded with coordinator corrections
to a scout's mistaken mailbox path and default-runtime-flavor observations.

The private Redis container and temporary directories were removed. The same
45 pre-existing container identities/states remain. Registered timer waker cleanup
is covered; complete native/poll backend-thread shutdown is not accepted.

## Scope and evidence

Effective lines, before -> after: watcher 163 -> 162; runtime 265 -> 255;
timer 22 -> 27; timer tests 51 -> 98; mailbox 133 -> 119; mailbox tests 135 -> 134.
All six owners stay below 500. The legacy folder-sync root remains byte-identical
at 5553. Strict remains 2112 scanned / 12 hard / 20 mandatory / 39 soft; all
32 files above 700 retain their hashes/counts. Clearance is 113/145 (77.9%).

Evidence: target/effective-line-evidence/20260916-folder-sync-owned-timer/.
The input union contains 1817 files, with 1811 unchanged neighbors and 22 unchanged
web/Tauri assets. Source snapshots, original files, gate receipts, cleanup,
reconstruction, reviews and scripts are hash-bound. scope.json observedAt:
2026-09-15T19:06:12.158Z. Scope SHA-256:
b3cc338471f608e2da373c972c9ef381f90a7ec0e32d9ba0ca16e38b4db71480.

publication.json verifies the five final documents and exact Git deltas. Gateway
retains 193 modified, 1 unstaged deletion, 2 staged deletions and 2334 untracked
entries; Neuro retains 10 modified and 182 untracked. Gateway HEAD remains
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d; Neuro HEAD remains
bf818f0324024634bc890585efb78cc8e603d11a. No dependency, policy, baseline, exception,
profile, persistent service, staging, commit or release change occurred. Modified
source and documentation are UTF-8 without BOM.

## Remaining requirements

Rapid boolean watch coalescing, filesystem events retained across disable,
status read-modify-write races, startup I/O deadlines and full backend shutdown
remain lifecycle work. Timer tests cover the owned Sleep boundary; the Redis
fixture has no PostgreSQL pool and does not prove credential import/export.
Deletion-path storage bounds, concurrent directory replacement, explicit/missing
database deletion, remaining root migration and full strict/feature/provider/
runtime/UI/Docker/release acceptance remain open. S06 original source/cursor/final
build remains reserved under GWP-20260912-01. Persistent service target remains
4200; the only release root is Neuro/release/Gateway. The overall goal stays active.
