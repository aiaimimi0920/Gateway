# ChatGPT Web response byte admission

Coordinator-owned, 2026-09-15. State: accepted bounded whole-body reads.
Six web reverse reads now reuse the unchanged 64 MiB provider-aware charset/BOM
collector: bootstrap, requirements, prepare, accumulated conversation, streaming
non-success and successful non-SSE diagnostics. Live SSE stays direct. Request
headers/order, timeout floors, protocol parsing and normal classification remain.

Production bootstrap/execution/requirements/transport owners measure 259/191/161/170.
Test entry 422; body_wire/body_targets/body_limits 130/100/171. All remain below 500.
The contract limits accumulated response bytes; total decoded/process RSS and
cross-request concurrency are not bounded by this checkpoint.

Twelve real loopback declared/chunked overflow cases fail before, pass after and
require rejection before EOF. Exact-limit, HTTP charset and first SSE output before
upstream completion pass in both phases. Baseline 98 passed / 12 failed; candidate
110/110. Shared reader 12/12 paired. All-targets, scoped rustfmt, source proof,
checker 19/19, ratchet and both Git checks pass under serialized native gates.
Strict 2087/12/20/40; 32 above 700; clearance 113/145 (77.9%).

Evidence: target/effective-line-evidence/20260915-chatgpt-web-body-bounds/.
Scope SHA-256: 3d5a75a738f3ded7eedeb535d36784e8b42514a6c0a6c01528b14310fbe4bd68.
[Full checkpoint](../../status/2026-09-15-chatgpt-web-body-bounds.md).
Official API reads, scheduling, real-provider and broader release gates remain.
S06 original implementation/cursor and final release-build coordination stay
reserved under GWP-20260912-01. Full optimization and release remain open.
