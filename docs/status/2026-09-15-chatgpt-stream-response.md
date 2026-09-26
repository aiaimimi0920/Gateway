# ChatGPT non-SSE stream response repair, 2026-09-15

The inherited successful non-SSE response panic is reproduced and repaired.
execute_stream now returns a fixed ServerError/HTTP 500 with provider identity,
retryable classification and code chatgpt_web_non_sse_response after the existing
response classifier accepts an ordinary 2xx non-SSE response. The diagnostic
contains no upstream body or Content-Type value. Challenge, invalid-session,
non-2xx and normal SSE handling keep their previous behavior.

Production changes are one replacement in src/upstream/chatgpt/execution.rs:
180 -> 184 effective lines. Existing execution/tests.rs adds only mod stream,
418 -> 419. New execution/tests/stream.rs owns nine focused tests at 187 lines.
All scoped files remain below 500; no additional large-file clearance is claimed.

## Verification

The accepted paired command used unchanged test sources:

    cargo test --offline --locked --lib chatgpt:: -- --test-threads=1

Both phases used GATEWAY_PREBUILT_WEB_UI=1. Baseline3: 90 passed / 5 failed.
Each failure hits the original non-SSE unreachable assertion. Candidate: 95/95,
zero failed/ignored, with 2932 filtered out. All 86 predecessor test identities
remain passing, with unchanged warnings. Tests cover 200 JSON, 200 plain text,
missing Content-Type, 204 and fixed diagnostics for synthetic private material.
Four preservation tests cover 200 browser challenge, 401 invalid session, ordinary
503 and consumable canonical SSE, including two case/parameter header forms.

The new tests invoke production execute_stream through ephemeral loopback HTTP
with proxy use disabled. They reuse requirements/prepare fixtures, consume Paris,
stop and one final DONE frame, and bound stream consumption. Normal paths abort
and await their owned server task; Drop also aborts on a panic. No live provider,
profile, credentials, existing service or release is involved.

The initial compile attempt used the nonexistent Unauthorized test enum variant.
It was corrected to the existing Authentication variant. The first runtime attempt
then reproduced all five panics plus one incorrect 503 preservation expectation.
Current status-first classification maps ordinary 503 to ServerError; that test
was corrected before the accepted baseline3/candidate pair. Both initial attempts
are preserved and excluded from paired acceptance. No production code was changed
to accommodate either fixture correction.

Default cargo check --offline --locked --all-targets, scoped official rustfmt
--check, checker tests 19/19, ratchet and both staged/unstaged Git diff checks pass.
Two independent read-only reviews found no introduced defect. Native gates ran
serially against frozen inputs; all owned sessions are terminal and process guards
are idle. Exact source proof preserves every byte except the production replacement
and test module declaration, including the original three execution test bodies.

## Evidence and remaining work

Acceptance: target/effective-line-evidence/20260915-chatgpt-stream-response/scope.json.
Observed at 2026-09-15T11:46:21.327Z; SHA-256:
73d38584cd7cfc6bf95b200c4359ed3f8791d17cdf1d64f14fc486bc609f0d9f.
Frozen input union: 1789; unchanged neighbors: 1786; unchanged web/Tauri assets: 22.
Original source, every failed/passed log, receipts, snapshots and independent review
remain beside the scope. publication.json verifies final documents and Git status.

Strict remains expected exit 1: 2084 scanned, 12 hard, 20 mandatory, 40 soft.
32 files remain above 700: 20 Rust and 12 runtime-profile/vendor files.
Clearance stays 113/145 (77.9%). No checker, policy, baseline or exception changed.
Gateway HEAD remains 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 188 modified,
one unstaged deletion, two staged deletions and 2281 untracked paths at scope.
This report adds one untracked path at publication. Neuro HEAD remains
bf818f0324024634bc890585efb78cc8e603d11a: 10 modified and 182 untracked paths.
Exact comparisons preserve all inherited repository state; no commit occurred.

Whole-body read limits, MIME matching, synchronous solver scheduling, real-provider
compatibility and remaining strict/feature/language/provider/packaged runtime/UI/
Docker/release gates remain open. Browser-refresh and escalation policy are unchanged.
The next response hardening work should establish bounded body-read contracts
before changing allocation or timeout behavior. S06 original Rust/Gemini ownership,
its cursor and final release-build coordination stay reserved under GWP-20260912-01.
Persistent target stays 4200, with no persistent 4226. Full optimization stays active.

