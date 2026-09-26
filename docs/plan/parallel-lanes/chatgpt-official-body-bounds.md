# ChatGPT official API response byte admission

Coordinator-owned, 2026-09-15. State: accepted bounded whole-body reads.
Four official executor reads use existing 64 MiB provider-aware collectors:
successful JSON and the three non-success diagnostic paths. JSON keeps byte-based
rquest decoding; diagnostics preserve HTTP charset/BOM and ordinary unreadable
fallback. Resource admission errors propagate with the original provider.
Successful raw streaming and the existing Responses accumulator remain unchanged.

Entry 370 -> 362 effective lines; response-body owner 44; test owners 90/250/86/139.
All changed owners remain below 500. Nine held-open overflow cases fail before and
pass after, including Codex provider retention. Original thirteen test identities
and twelve new preservation tests pass in both phases. Baseline 25 passed / 9 failed;
candidate 34/34. Responses accumulator 3/3 and shared reader 12/12 paired.
All-targets, scoped rustfmt, exact source proof, checker 19/19, ratchet and both Git
checks pass under serialized native gates. Strict 2092/12/20/40; 32 above 700;
clearance 113/145 (77.9%).

Evidence: target/effective-line-evidence/20260915-chatgpt-official-body-bounds/.
Scope SHA-256: 4e2921786f11aedec2696b33b76527983f70c4bbced59ebf27bf8aa0a9f3c832.
[Full checkpoint](../../status/2026-09-15-chatgpt-official-body-bounds.md).
The byte cap does not impose a total decoded/allocator/process memory or aggregate
concurrency budget. MIME matching, scheduling, provider compatibility and broader
release gates remain open. S06 original Rust/Gemini implementation/cursor and final
release-build coordination remain reserved under GWP-20260912-01.
