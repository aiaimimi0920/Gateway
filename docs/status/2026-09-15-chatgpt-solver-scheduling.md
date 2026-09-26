# ChatGPT proof solver scheduling, 2026-09-15

The bounded async scheduling boundary is verified. Full Gateway optimization,
real-provider load thresholds and runtime/UI/Docker/release acceptance remain open.

## Implemented behavior

Legacy PoW, required PoW and Turnstile previously executed synchronously inside
get_requirements. A private solver owner now offloads those computations with
spawn_blocking. Shared process-wide two-slot try-admission happens before cloning
worker inputs. Saturation or closed admission returns a fixed ServiceUnavailable
error, provider chatgpt_web_reverse_compatible, code chatgpt_web_solver_busy. This is a new
fail-fast admission policy, not a measured provider throughput threshold.

The worker owns its inputs and semaphore permit. Dropping the caller aborts queued
blocking work. An already-running synchronous solver retains its resources until
actual completion or unwind. A worker JoinError returns fixed ServerError text,
provider and chatgpt_web_solver_worker_failed; original solver errors pass through.

A supplied nonempty Turnstile token still bypasses dx computation. The empty-key
attempt still precedes legacy-token fallback. Existing PoW/VM work budgets, pure
protocol solvers, HTTP/JSON processing, Arkose handling, request fields and final
ChatRequirements assembly are preserved. User-agent copies are deferred until
admission. The two-slot limit bounds admitted queued/running jobs and their owned
inputs; total process memory, retained completed results, aggregate HTTP requests
and wall-clock latency are not bounded by this change. No hard solver deadline or
mid-running cooperative/forced cancellation is implemented.

Effective-line measurements:

- execution/requirements.rs: 161 -> 159.
- execution/requirements/solver.rs: new 71 at the inline baseline, 85 after offload.
- execution/requirements/solver/tests.rs: new 101.
- execution/requirements/solver/lifecycle_tests.rs: new 130.

All completed owners remain below 500. No above-700 file is cleared by this lane.

## Verification

The predecessor HTML checkpoint already verified the unchanged 170-test ChatGPT
filter. This lane adds an instrumented inline seam with admission-before-copy and
eight tests. That baseline already has admission scaffolding; its results isolate
scheduling, panic handling and resource lifetime rather than claiming the old
unmodified implementation had the new admission policy.

Both fresh runs used GATEWAY_PREBUILT_WEB_UI=1 and:

    cargo test --offline --locked --lib chatgpt -- --test-threads=1

- Instrumented inline baseline: 174 passed / 4 failed, 2905 filtered out.
- Frozen candidate: 178/178, 2905 filtered out; first candidate run passed.
- All original 170 test identities remain green. Four new admission/protocol tests
  pass in both phases. Both test owners and requirements wiring are byte-identical
  across baseline and candidate.

The four failures are exactly solver_runs_off_executor,
solver_panic_returns_fixed_error_and_releases_capacity,
cancelled_queued_solver_does_not_execute and
cancelled_running_solver_retains_input_and_capacity. Thread identity directly
checks off-executor work. A saturated one-thread blocking pool exposes queued
cancellation; held-worker signaling verifies running inputs/capacity survive caller
drop. Release-on-drop and bounded five-second waits protect fixture teardown.
Legacy JSON/user-agent, PoW success/original error, Turnstile empty-key/legacy
fallback and admission-before-prepare have focused preservation tests. Neither
baseline nor candidate elapsed test time is a performance benchmark.

Closing gates pass: cargo check --offline --locked --all-targets, scoped official
rustfmt --check, checker tests 19/19, ratchet, and Gateway/Neuro staged and unstaged
git diff --check. All native phases are terminal and process guards are idle.
The inherited Gemini unused HashMap warning remains. Strict audit exits 1 for
existing debt: 2097 scanned / 12 hard / 20 mandatory / 39 soft. Above-700 inventory
is unchanged at 32 files; clearance remains 113/145 (77.9%). No checker, policy,
baseline, exception or dependency changes were made.

## Evidence and remaining work

Evidence: target/effective-line-evidence/20260915-chatgpt-solver-scheduling/.
The before capture reverified the HTML scope, five published docs, 1799 inputs,
22 assets and exact repository states. Final union is 1802 inputs, with 1798
unchanged neighbors and 22 unchanged web/Tauri assets. Source proof reconstructs
requirements from the hashed original file and the worker from the hashed inline
baseline, using exact replacements and official rustfmt on stdin. Test hashes
remain fixed. Gate logs, receipts, snapshots, process guards, inventories, review
and evidence scripts are bound by scope/publication hashes.

Independent fixture and semantic reviews found no introduced defect. The semantic
reviewer could not locate ignored evidence snapshots through search, so it did not
provide a byte-level original diff; the coordinator projection supplies that proof.
No patch, formatter or executed-fixture correction was needed in this lane.
All modified/new source, evidence scripts and docs are UTF-8 without BOM.

scope.json observedAt: 2026-09-15T14:41:58.830Z.
Scope SHA-256: d2352887eb35289039b5c7e08df3b82cbfb6ac65f9b3a7401adcf9e1e73dcb22.
Source SHA-256 values:

- requirements.rs: 96abdd93dad3349f07d3d08fb11604c2249e86fa2a507f63af028bf66941ff15.
- solver.rs: 34661381b2913ec79b1c9b47b77e042ba058581730b9f887e4255fe021547f3c.
- tests.rs: 333fc55ab0ce0948b5e031298866d416f991b72e05b386f999f2f40e688c35bb.
- lifecycle_tests.rs: 05510c1659b7e621bca3e650a1241c22e4e00fabbc3758ce942881f51a615ab5.

publication.json checks all final docs, source/evidence integrity, script limits
and exact Git delta. Gateway: 192 modified, 1 unstaged deletion, 2 staged deletions,
2305 untracked; Neuro: 10 modified, 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit or release occurred.

Remaining strict debt, full feature/language/provider/runtime/UI/Docker/release
checks and real-provider concurrency tuning remain open. Existing body sniffing
stays heuristic. S06 original Rust/Gemini implementation, cursor and final-build
coordination remain reserved under GWP-20260912-01. Persistent service target stays
4200; releases belong only under Neuro/release/Gateway.
