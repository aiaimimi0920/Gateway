# Folder synchronization startup activation

S09 disabled-at-start activation is accepted on 2026-09-16. The existing task
subscribes before status I/O and waits for the latest enabled value. Management
enablement now starts the configured runtime without a restart. The same receiver
continues into the existing loop; no supervisor, extra task or polling is added.
Unconfigured-root exit and enabled startup retain their behavior.

Isolated Redis baseline2: 2 passed / 4 failed. Frozen candidate: 6/6, including a
real filesystem event, activation without watchers, an enable-during-status-write
connection barrier and cancellation/AppState/directory cleanup. Original library
56/56 identities remain unchanged. The first baseline's DEL argument compile error
is retained; only the fixture argument changed from array to Vec before baseline2.
No assertion or candidate correction was needed. Foreign Beaver compilation
interrupted terminal cleanup observation; idle recovery retained the passing tests.
All three fixture containers/directories were removed; 45 existing containers kept
their identities/states. No successful database sync or native-only event claim.

All-targets, scoped fmt, source reconstruction, checker 19/19, ratchet and both
Git checks pass. Runtime 257 -> 265; tests/fixture 96/178. Root unchanged 5553.
Strict 2112/12/20/39; 32 above 700; clearance 113/145 (77.9%). Union 1817;
unchanged neighbors 1814 and assets 22. No dependencies/policies/baselines,
profiles, persistent services, staging, commits or releases changed.

Next: stale queued timers and rapid toggles, backend shutdown and status races;
then remaining root, strict, runtime/provider/UI/Docker/release acceptance.
S06 original source/cursor/final build remains reserved under GWP-20260912-01.
Evidence: target/effective-line-evidence/20260916-folder-sync-startup/.
Scope SHA-256: 70273d7287e4e7287658a3a0efbf8f65960947dd49bc727653368b047016aa02.
[Report](../../status/2026-09-16-folder-sync-startup.md).
