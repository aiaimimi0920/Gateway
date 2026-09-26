# Folder synchronization enable operation ownership

S09 per-runtime management admission and caller-cancellation ownership are verified
on 2026-09-16. An owned mutex permit is acquired before payload copies or spawning.
It covers the complete override -> memory -> status operation. Cancelled waiters
produce no work; admitted work continues after caller drop, logging only typed
error kind. No whole Config/AppState clone, new key or distributed transaction.

Staged inline baseline: 9 passed / 1 failed. Frozen real Redis candidate: 10/10,
zero ignored. Only the task-ownership boundary changes between runs. Four new
control tests cover admitted cancellation, a cancelled waiter, admission order and
status-error release/recovery. All six original runtime tests and fixture remain
unchanged. Library 63/63 with six unchanged CAS tests ignored in this run. Those
CAS tests retain the preceding checkpoint's explicit acceptance, without replay.

All-targets, scoped formatter, complete source reconstruction, checker 19/19,
ratchet and both repositories' Git checks pass. Two fixture containers/directories
are cleaned; all 45 existing containers keep identity/state. No candidate/fixture
correction. State/status/runtime target: 141/140/98; config/enable/tests: 44/60/134.
Root remains 5543. Strict 2120/12/20/39; 32 unchanged files above 700; clearance
113/145 (77.9%). Union 1825, unchanged neighbors 1819, unchanged assets 22.

Next: rapid boolean transitions, queued filesystem events, operation deadlines and
backend shutdown. Other-process/direct-setter/shared-field ordering, ambiguous I/O,
database synchronization, root migration and full release acceptance remain open.
S06 original source/cursor/final build stays reserved under GWP-20260912-01.
Evidence: target/effective-line-evidence/20260916-folder-sync-enable-ownership/.
Scope SHA-256: 54e65d9a03e8a1d9c4e2762c048da01a07b443e474f9c0c078983cde2feececc.
[Report](../../status/2026-09-16-folder-sync-enable-ownership.md).
