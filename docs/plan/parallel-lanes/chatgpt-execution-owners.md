# ChatGPT execution ownership

Coordinator-owned independent ChatGPT structural scope, accepted 2026-09-15.
Only src/upstream/chatgpt/execution.rs and its execution/{requirements,transport,
policy,tests}.rs children are production/test write scope. S06's original Rust/
Gemini implementation, cursor and final build remain GWP-20260912-01.

Baseline: execution.rs has 1067 effective lines. Preserve both public execution
entry points, original function bodies, request/preflight/prepare order, aliases,
headers, timeouts, proof/Turnstile call sites and all three loopback test identities.
Keep orchestration in the entry, requirements acquisition, HTTP transmission and
conversation policy in cohesive private children; move the original test module
without altering assertions. Do not consolidate the duplicate orchestration in
this structural checkpoint. Every completed owner must remain <=500 lines.

Run the same complete chatgpt:: library filter before/after under prebuilt web mode,
then all-targets, scoped official rustfmt, source proof, checker tests/ratchet,
strict inventory and both repository Git checks. Freeze all accepted inputs/assets
and serialize native gates with process guards. No provider/profile/service,
dependency, policy/baseline/exception, release or commit operation.

Evidence: target/effective-line-evidence/20260915-chatgpt-execution-owners/.
Entry 1067 -> 180; requirements/transport/policy/tests 158/167/182/418.
Paired complete ChatGPT library tests pass 86/86, preserving 65 protocol and 21
upstream identities with identical warnings. All-targets, scoped rustfmt, exact
fourteen-function/three-test source proof, checker 19/19, ratchet and both staged/
unstaged repository Git checks pass. Two read-only reviews found no introduced
defect. All native handles are terminal and all 22 web/Tauri assets unchanged.

Strict 2083/12/20/40; 32 above 700; clearance 113/145 (77.9%). Union 1788;
unchanged neighbors 1783. Scope SHA-256:
b696478dd56eb94353884e7eba2f906a490a786858b5398cc0abda80e2b554dd.
[Detailed acceptance](../../status/2026-09-15-chatgpt-execution-owners.md).

The inherited successful non-SSE stream panic needs separate loopback reproduction
and repair. Full strict/provider/runtime/release work and S06 remain open.
