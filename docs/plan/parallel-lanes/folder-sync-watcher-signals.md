# Folder synchronization watcher signals and timer ownership

S09 coalesced signals and pending-timer cancellation are accepted on 2026-09-16.
The mailbox holds at most three signal-kind nodes, unions every distinct deletion
path, keeps the latest error diagnostic and orders kinds by their latest occurrence.
Callbacks avoid capacity waits; mutex contention/path merging remain synchronous.
Receiver Drop clears pending storage outside the lock; last-sender drop wakes recv.
DebounceTask Drop aborts pending work when its parent is cancelled; the original
first-event schedule and explicit abort behavior remain unchanged.

Baseline 51 passed / 5 failed; same candidate 56/56. Original 45 test identities and
all eleven new tests/fixtures remain exact. Four sender threads preserve 400 paths;
10,000 duplicate changes occupy one node. All-targets, scoped fmt, source proof,
checker 19/19, ratchet and both Git checks pass. No candidate or proof correction.

Watcher/runtime/mailbox/debounce/tests effective lines: 163/257/133/22/135/51.
Root unchanged 5553. Strict 2110/12/20/39; 32 above 700; clearance 113/145 (77.9%).
Union 1815; unchanged neighbors 1809; unchanged assets 22. No dependencies/policies,
baselines, profiles, services, staging, commits or releases changed.

Coalescing intentionally reduces intermediate status writes and error diagnostics.
Distinct deletion-path bytes remain data-sized; reliable overflow/storage is needed
for a total-memory hard cap. No deletion paths are discarded or ancestors invented.
Next: enable-after-disabled-start, stale queued timer/toggle behavior and native
backend shutdown, followed by remaining root and full runtime/release acceptance.
S06 original source/cursor/final-build reservation remains GWP-20260912-01.

Evidence: target/effective-line-evidence/20260916-folder-sync-watcher-signals/.
Scope SHA-256: b0d6433a7f0079785c193d1f7c45f48c92ab1a8a6d3652e92c974f97862b3400.
[Report](../../status/2026-09-16-folder-sync-watcher-signals.md).
