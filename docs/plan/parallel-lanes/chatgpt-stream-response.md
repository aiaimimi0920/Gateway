# ChatGPT stream response boundary

Coordinator-owned lane, 2026-09-15. Predecessor: accepted ChatGPT execution owners.
State: accepted. Scope: execution.rs non-SSE failure, tests.rs wiring,
and execution/tests/stream.rs. Before repair, the production entry measured 180
effective lines and the existing test entry 418. New owners must stay below 500.

Reproduce successful JSON/text/missing-content-type/204 responses reaching the
inherited panic. Return a fixed server error with code chatgpt_web_non_sse_response,
without copying upstream body or header values into diagnostics. Preserve existing
challenge/session/non-2xx classification and consume normal translated SSE output.
No classifier, content-type matching, fallback policy, protocol or timeout changes.

Matched frozen ChatGPT library tests run before/after, followed by all-targets,
scoped formatter, checker tests, ratchet, strict inventory and both Git checks.
Native gates are serialized with frozen source and web assets. Evidence root:
target/effective-line-evidence/20260915-chatgpt-stream-response/.

S06 original Rust/Gemini implementation, cursor and final release-build coordination
remain reserved under GWP-20260912-01. Full optimization and release remain open.

Accepted baseline3 90 passed / 5 production panics; frozen candidate 95/95.
Original 86 tests and four new preservation tests pass in both accepted phases.
Entry 180 -> 184; existing/new test owners 419/187. All-targets, scoped rustfmt,
exact source proof, checker 19/19, ratchet and both Git checks pass. Two read-only
reviews found no introduced defect. Initial enum/503 fixture corrections remain
recorded separately. All native phases are terminal; 22 web/Tauri assets unchanged.
Strict 2084/12/20/40; 32 above 700; clearance 113/145 (77.9%).
Scope SHA-256: 73d38584cd7cfc6bf95b200c4359ed3f8791d17cdf1d64f14fc486bc609f0d9f.
[Acceptance report](../../status/2026-09-15-chatgpt-stream-response.md).
