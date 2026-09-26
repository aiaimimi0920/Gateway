# Folder synchronization blocking run owner

The S09 blocking run owner checkpoint is verified on 2026-09-16. Existing per-runtime
admission precedes owned input preparation and shared-pool queueing. One blocking
worker drives the complete operation through Handle.block_on, retaining the permit
through filesystem/JSON/hash work, database waits and final status. Normal callers
await completion; caller drop and auto-mode disable do not cancel admitted work.

A runtime-owned monitor wakes blocked async waits when the Tokio runtime shuts
down. Native calls retain admission until they finish. This permits async teardown
without claiming native preemption, graceful drain or guaranteed final status.
JoinHandle drop detaches; the review's contrary claim is rejected against pinned
Tokio source and cancellation tests. The blocking slot remains occupied during
database waits; shared-pool contention and throughput are not measured.

Unchanged-production baseline: 102 passed / 3 failed / 6 ignored. Frozen candidate
library: 107/107, six ignored, all 108 predecessor identities exact. Three scheduling
regressions are repaired; two shutdown cases are explicitly candidate-only safety
tests added after baseline. Existing cancellation assertions remain, with bounded
admission/fixture drains. All baseline-executed test files retain exact hashes.

Real Redis baseline/candidate: 15/15, all prior identities/results exact. Deletion
9/9, all-targets/fmt/checker 19/19/ratchet/both Git checks pass. Owned fixtures and
temp roots cleaned; all 45 prior containers preserved. No post-freeze candidate
correction, successful native replay or evidence-script correction was needed.

Owner/unit-tests/blocking-tests/runtime-tests/shutdown-tests effective lines:
54/107/86/146/97. Only owner.rs changes production behavior; run/state/status and
all lower filesystem/import/export bodies remain hash-exact. All 61 subtree Rust
files <=500, maximum 460. Strict 2163/11/20/39; all 31 above-700 entries unchanged;
clearance 114/145 (78.6%). Union 1868, unchanged neighbors 1863, assets 22.

Next: watcher startup mkdir/notify ownership and management deletion blocking.
Successful PostgreSQL/status persistence, shared-pool fairness, input/DB/parsed-memory
budgets, deadlines/graceful shutdown, races/hard links/cross-process exclusion and
full runtime/provider/UI/Docker/release acceptance remain open. S06 stays reserved;
the overall optimization goal is active.

Evidence: target/effective-line-evidence/20260916-folder-sync-blocking-owner/.
Scope SHA-256: 2be339b75d3604f4e83ca807e4dfeb27c2dd7075045f61bb6539a55fb227583d.
[Report](../../status/2026-09-16-folder-sync-blocking-owner.md).
