# ChatGPT proof solver scheduling

Status: verified, coordinator-owned, 2026-09-15.

Accepted inline baseline: 174 passed / 4 failed; frozen candidate: 178/178. Original
170 identities and four new preservation/admission tests remain green. Requirements
161 -> 159; solver/test owners 85/101/130. All-targets, scoped rustfmt, source proof,
checker 19/19, ratchet and both repository Git checks pass. Strict 2097/12/20/39;
32 above 700; clearance 113/145 (77.9%). All native phases are terminal.
[Acceptance report](../../status/2026-09-15-chatgpt-solver-scheduling.md).

## Design and regression contract

Predecessor HTML MIME scope: 9ab51c1137bf094edf1abd13f3a801d1f11edbf336757a7250c27bfa63ddb4ea.
Before snapshot verifies 1799 source inputs, 22 assets, five docs and exact Git
states. Requirements owner begins at 161 effective lines. Write scope is that
owner and new requirements/solver.rs plus its two test owners.

Legacy PoW, required PoW and the two-attempt Turnstile solver previously ran inline
inside get_requirements. Use a private shared scheduling boundary with two admitted
queued/running jobs process-wide. Reject saturation/closure before copying worker
inputs; mandatory work returns a fixed provider-tagged busy error, never silent
skip. Preserve protocol errors and token semantics. The two-slot budget follows
the existing bounded CPU-job pattern and is not a throughput benchmark claim.

Regression scaffolding routes real solvers through an inline seam with admission.
Its scheduling/panic/cancellation tests must fail before offloading. This baseline
is an explicitly instrumented inline boundary, not a claim that the unmodified old
implementation already had admission. The prior unchanged source passed the broad
170-test ChatGPT filter in the predecessor checkpoint.

Candidate uses spawn_blocking with the permit held by the worker. An abort-on-
drop handle cancels queued work; running synchronous solvers retain inputs/capacity
until actual completion. No forced termination or cooperative mid-solver cancellation
is claimed. Existing PoW/VM work budgets remain unchanged. Busy admission creates
no unbounded application queue; at most two owned inputs/jobs are admitted.

Use current-thread thread identity and held-worker signaling tests, saturation
before-prepare checks, panic/error mapping, running cancellation and a saturated
blocking-pool queued-cancellation test. Keep cleanup bounded via release-on-drop
and five-second fixture waits; do not treat timing as a benchmark. Real solver
wrapper tests preserve legacy JSON, PoW success/failure and Turnstile key fallback.

All source/tests/assets freeze during serialized native runs. Run the broad chatgpt
library filter, all-targets, scoped formatter, checker tests, ratchet, strict and
both repository Git checks. All new/modified owners must remain <=500 lines.
S06 original scope and final native release-build coordination remain reserved.
Full optimization, real-provider thresholds, runtime/UI/Docker/release remain open.
