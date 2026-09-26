# Folder synchronization run ownership

S09 run admission and cancellation ownership are verified on 2026-09-16. One
per-runtime try-lock admits synchronization before owned input copies. Busy calls
return HTTP 409/provider_credential_folder_sync_run_busy without queueing. An owned
async task retains the permit through filesystem/DB/status completion after caller
cancellation or auto-mode disable; normal callers await its full result.

Manual/HTTP, all watcher branches and refill share the owner. Immutable folder
config, pool handles and explicit paths cross the boundary; AppState/unrelated
configuration do not. Existing phase/error ordering and successful-only deletion
intent clearing remain. Every final status CAS retry reads fresh runtime enabled.

Real Redis baseline: 12 passed / 3 failed. Frozen candidate: 15/15, all ten original
identities retained. Library 102/102, six ignored, all 103 predecessor identities
exact; five new ownership tests pass. Deletion 9/9, all-targets/fmt/checker 19/19/
ratchet/Git pass. Both Redis fixtures removed, all 45 prior containers preserved,
no owned temp roots. No source correction or native success replay. A proof-script
marker correction is retained separately from the successful native evidence.

Scoped root/state/run/owner/unit-tests/status/config/status-run/runtime-entry/
runtime-tests: 59/170/122/33/96/149/54/52/100/136. All 59 subtree Rust files <=500,
maximum 460. Strict 2161/11/20/39; 31 unchanged above 700; clearance 114/145 (78.6%).
Union 1866, unchanged neighbors 1856, assets 22.

Next: blocking filesystem ownership/offload while retaining run capacity through
native completion. Duration/deadlines/shutdown/status, DB hydration/parsed-memory
budgets, successful PostgreSQL/provider validation, races/hard links, cross-process
exclusion and full runtime/UI/Docker/release acceptance remain open. S06 remains
reserved; the overall goal is active.

Evidence: target/effective-line-evidence/20260916-folder-sync-run-ownership/.
Scope SHA-256: b28d345ae5be74ae30b8366b781e04402eb55ecb2de8dddc7087c3fd67a62d2e.
[Report](../../status/2026-09-16-folder-sync-run-ownership.md).
