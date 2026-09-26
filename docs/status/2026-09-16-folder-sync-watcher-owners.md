# Folder synchronization watcher ownership, 2026-09-16

S09 watcher ownership and exact task migration are verified. The root remains
legacy debt; watcher runtime hardening and full Gateway acceptance remain open.

## Implemented boundaries

watcher.rs owns the event/signal/handle types, their two methods, the native-plus-poll
constructor, both backend constructors and the two event/path filters. runtime.rs
owns the complete original start_folder_sync_task body: startup, initial/periodic
sync, runtime toggles, event accumulation, debounce scheduling and watch status.
watcher/tests.rs owns the three original filter tests without rewritten assertions.

The public root start_folder_sync_task path remains available through reexports;
src/runtime.rs requires no edit. Only folder_sync_path_should_trigger gains
pub(super), and the root retains a private alias for the deletion owner's existing
import. Deleted-path extraction and watch-status persistence remain in the accepted
deletion/status owners. Root Duration remains because JSON-read retry still uses it.
All sync/import/export functions, shared counters and direction stay in the parent.

Native and poll both start as before. Their callbacks, recursive modes, minimum
poll interval, error messages, ignored send failures and handle ownership remain.
The task retains initial and subsequent sync call order, status writes, minimum
interval/debounce delay, pending-path clearing/retention and existing abort behavior.
This is a structural checkpoint with no intentional behavior or dependency change.

Effective lines: root 5975 -> 5553, a reduction of 422; watcher 161; runtime 257;
original-test owner 30. The runtime file owns one complete lifecycle. All three new
owners are below 500. Combined S09 root reduction is 6655 -> 5553, or 1102 lines.
No cleared-file count increases while the root remains above 700. Clearance stays
113/145 (77.9%). No checker/policy/baseline/exception or runtime profile changed.

## Fresh verification

Paired baseline and first frozen candidate used GATEWAY_PREBUILT_WEB_UI=1:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1

Both pass 45/45 with 3038 filtered out. Exactly three filter tests change module
path from provider_credential_folder_sync::tests to its watcher::tests module.
The other 42 identities remain unchanged: 36 parent tests and six deletion tests.
All original assertions and shared fixture bodies reconstruct from the captured
original file. No test behavior was rewritten and no synthetic duplicate was added.

The source verifier reconstructs all four complete files from the original root,
allowing only ownership/import/reexport changes and the single predicate visibility
change, and requires exact official-rustfmt output equality. All eight moved
functions/methods and all three type definitions retain their bodies.

The first proof capture stopped before scope.json because its expected projection
removed import text without its newline, producing two artificial blank lines.
The native patch had correctly removed complete lines. The first verifier and
process guard remain archived; the corrected verifier removes the exact newline
and still requires full formatted equality. No source/test correction, weakened
comparison or repeated native test was used. See source-proof-recovery.md.

All-targets cargo check, scoped official rustfmt --check, checker tests 19/19,
ratchet and both Gateway/Neuro staged and unstaged Git checks pass. Native phases
are serialized and terminal. Baseline/candidate warning sets match; the inherited
Gemini unused HashMap warning remains. Strict exits 1 for existing debt: 2106 scanned /
12 hard / 20 mandatory / 39 soft. Above-700 membership remains 32; only this root's
line count changes. The previous static-deletion sources and its 9/9 integration
fixture are byte-identical; that integration gate was not rerun for this extraction.

## Evidence and repository state

Evidence: target/effective-line-evidence/20260916-folder-sync-watcher-owners/.
Before capture revalidated the preceding containment scope/publication, all 1808
inputs, 22 assets, five docs and both exact Git states. Final union: 1811 inputs;
1807 unchanged neighbors and 22 unchanged web/Tauri assets. Existing status, deletion,
path containment, public deletion tests and shared fixtures retain their hashes.

scope.json observedAt: 2026-09-15T16:40:19.544Z.
Scope SHA-256: 2123f0daf1f0b5b75665aa6382769c7e9f49a81fa9b399fcedef7d09e24ebfbf.
Source SHA-256 values:

- provider_credential_folder_sync.rs: 8ecf2b16f6122d136eaf6c8070baef35a25827fd9ab29f180639587674182900.
- watcher.rs: f097467bec8129dc3918065c76363386aaf717f6106519bb797903d294b32316.
- watcher/runtime.rs: 993d08e6b75f3ad9bda09e0f2a0390367bf0a1e17eaca1d3bedfba79e4bd3cec.
- watcher/tests.rs: ccd8875a89e39bc176702ad916df902678157225f63b74428d85eca8a0401b47.

Source, snapshots, inventories, logs, receipts, guards, reviews and scripts are
hash-bound. publication.json verifies final five docs, evidence integrity, script
limits and exact Git deltas. Gateway final expected status: 193 modified, 1 unstaged
deletion, 2 staged deletions, 2322 untracked. Neuro remains 10 modified, 182 untracked.
HEADs stay 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. UTF-8 without BOM is verified. No staging,
commit, service change or release occurred.

## Runtime gaps and next work

Independent review found no introduced extraction defect. Existing tests establish
filter behavior and source preservation, not live backend/debounce/shutdown safety.
The next focused runtime work should reproduce event queue growth and timer cleanup:

- Callbacks use an unbounded channel while status writes and sync runs can delay
  receiving. Native and poll can both emit events for the same change.
- The timer coalesces from the first event; later events do not reset its deadline.
- Disable and DebouncedSync abort the timer, but dropping/returning from the main
  task does not explicitly abort the spawned timer's JoinHandle. Native watcher
  handles drop implicitly. Cancellation and live backend cleanup need regression proof.
- Disabled-at-start returns before subscribing to later toggles. Startup responsibility
  on subsequent enable needs investigation before behavior is changed.

No concurrent pending-set mutation was established: the receiver owns that state
and awaits sync inline while new events queue. Separately reproduce explicit/missing
DB deletion behavior before changing counters or transaction semantics. Static
file-deletion containment still has a concurrent-directory-replacement boundary.

The 5553-line root migration, strict/feature/language/provider/runtime/UI/Docker/release
gates remain open. S06 original source/cursor and final-build coordination remain
reserved under GWP-20260912-01. Persistent service target remains 4200; releases belong
only under Neuro/release/Gateway. Full optimization remains active.

