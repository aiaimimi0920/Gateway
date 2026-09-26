# Folder synchronization watcher ownership

S09 watcher/task/test ownership is accepted on 2026-09-16. Root 5975 -> 5553;
watcher/runtime/tests 161/257/30 effective lines. Event/signal/handle types, backend
constructors, filters and the complete background task retain exact bodies. Public
start_folder_sync_task is re-exported at the original root path. Only the path
predicate gains pub(super); its parent alias preserves the deletion import.

Paired broad folder_sync 45/45. Three original filter tests move module paths;
42 identities remain unchanged. Whole-root and all owner projections match official
rustfmt output. First proof capture corrected an import-newline reconstruction
error, with its original verifier retained; no candidate or fixture correction.
All-targets, scoped fmt, checker 19/19, ratchet and both Git checks pass.

Strict 2106/12/20/39; 32 above 700; clearance 113/145 (77.9%). Cumulative root reduction
from 6655 is 1102 lines, but root migration remains incomplete. Union 1811; unchanged
neighbors 1807 and assets 22. Static deletion containment source/tests remain exact.

Next: reproduce event queue growth and main-task cancellation/timer cleanup before
hardening those behaviors. Existing tests do not establish native backend, debounce,
Redis/DB or shutdown acceptance. Disabled-start/enable handling also needs review.
Root migration and full optimization remain open; S06/final build stays reserved
under GWP-20260912-01. No dependencies, policies, baselines, profiles or services changed.

Evidence: target/effective-line-evidence/20260916-folder-sync-watcher-owners/.
Scope SHA-256: 2123f0daf1f0b5b75665aa6382769c7e9f49a81fa9b399fcedef7d09e24ebfbf.
[Report](../../status/2026-09-16-folder-sync-watcher-owners.md).
