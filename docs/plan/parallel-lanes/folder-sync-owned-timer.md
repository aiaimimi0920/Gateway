# Folder synchronization runtime-owned timer

S09 queued-timer cancellation is accepted on 2026-09-16. The runtime owns one
optional pinned Sleep and selects its readiness directly. Disable drops even an
elapsed timer; no DebouncedSync message remains to cross re-enable. First-event
deadlines and cancellation-safe select borrowing are preserved. The timer branch
retains the latest-enabled check. No extra task or generation protocol is added.

Pre-change expired-abort probe: 3 passed / 1 failed. Candidate library: 60/60.
Seven owner tests replace three obsolete task/channel tests; 53 non-timer tests
remain with one mailbox-test rename. The old/new tests are not byte-identical.
Elapsed/new-window, deadline, one-shot and registered-waker cleanup contracts pass.
Unchanged isolated Redis runtime: 6/6. Fixture container/directories are removed;
45 existing containers retain identity/state. No PostgreSQL or backend-thread proof.

All-targets, scoped fmt, exact non-timer source reconstruction, checker 19/19,
ratchet and both Git checks pass. Owners: 162/255/27/98/119/134; root unchanged
5553. Strict 2112/12/20/39; 32 above 700; clearance 113/145 (77.9%). Input union
1817, unchanged neighbors 1811, unchanged assets 22. No dependency/policy/baseline,
profile, persistent service, staging, commit or release changes.

Next: rapid boolean toggles, queued filesystem events, status races and backend
shutdown; then remaining root and full runtime/provider/UI/Docker/release gates.
S06 source/cursor/final build stays reserved under GWP-20260912-01.
Evidence: target/effective-line-evidence/20260916-folder-sync-owned-timer/.
Scope SHA-256: b3cc338471f608e2da373c972c9ef381f90a7ec0e32d9ba0ca16e38b4db71480.
[Report](../../status/2026-09-16-folder-sync-owned-timer.md).
