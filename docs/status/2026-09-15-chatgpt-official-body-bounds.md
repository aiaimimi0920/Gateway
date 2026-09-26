# ChatGPT official API response byte admission, 2026-09-15

Four whole-body reads in the official API executor now reuse existing 64 MiB
provider-aware collectors: successful JSON and the three non-success diagnostic
paths (forced accumulation, nonstreaming and live streaming). Declared oversize
is rejected before collection; unknown-length oversize is rejected while reading,
without waiting for EOF. Resource failures retain their provider identity and
upstream_body_too_large or upstream_body_buffer_allocation_failed error code.

The new execution/body.rs owner keeps the two decoding contracts separate. JSON
uses bounded bytes and the existing rquest JSON decoder, without interpreting
HTTP charset. Diagnostics use the existing charset/BOM-aware collector. Ordinary
unreadable diagnostic bodies still fall back to `<unreadable body>` and the prior
HTTP status/body classifier. Resource admission failures bypass that fallback.
Successful raw HTTP streaming and the existing bounded Responses accumulator
remain unchanged, including status, headers and incremental body ownership.

The production/test entry changes from 370 to 362 effective lines (including four
new test module declarations). The new response-body owner has 44 lines. New
body_limits/body_preservation/body_targets/body_wire owners measure 90/250/86/139.
All changed owners remain below 500; no large-file clearance is claimed.

## Verification

The accepted baseline/candidate pair used identical frozen tests, serialized with
GATEWAY_PREBUILT_WEB_UI=1 and these commands:

    cargo test --offline --locked --lib upstream::chatgpt::official_api:: -- --test-threads=1
    cargo test --offline --locked --lib protocol::responses::tests::accumulate_tests:: -- --test-threads=1
    cargo test --offline --locked --lib protocol::upstream_body:: -- --test-threads=1

Official baseline: 25 passed / 9 failed; candidate: 34/34, zero failed/ignored,
3029 filtered out. All nine failures are the intended before-EOF overflow cases:
four read boundaries times declared/chunked framing, plus Codex provider retention.
The original thirteen official test identities and all twelve new preservation
tests pass in both phases. Responses accumulator tests pass 3/3 and shared reader
tests 12/12 in both phases, with matching identities and warnings. The candidate
passed its first native run; regression tests were not changed after baseline.

New tests use ephemeral no-proxy loopback servers and actual executor entrypoints.
Oversized declared/chunked responses stay open; exact-limit JSON is a valid small
object padded to 64 MiB. Fixtures use fixed 64 KiB writes, bounded request headers,
outer deadlines, and owned server abort/await cleanup. The Codex case exercises
the public forced-accumulation function with the Codex provider profile.

Preservation checks cover diagnostic windows-1252 decoding and retry timing,
truncated HTTP error fallback, valid UTF-8 JSON under a contradictory charset,
invalid UTF-8 and malformed/truncated JSON classification, Responses usage/tool
calls, and raw SSE status/headers/first bytes before upstream completion. The SSE
fixture releases its terminal frame only after the first output assertion.
Shared-reader tests also retain charset/BOM, cancellation and request-deadline proof.

Gateway default cargo check --offline --locked --all-targets, scoped official
rustfmt --check, checker tests 19/19 and ratchet pass. Gateway and Neuro both pass
staged/unstaged Git diff checks. Gates ran serially with frozen source/tests/assets;
all owned native sessions are terminal and process guards are idle. Independent
fixture and production reviews found no introduced defect in their inspected scope.
Exact source projection preserves the entry except four reads, import/module wiring
and the test declarations. The shared collectors, classifier and accumulator are
unchanged; inherited untracked state does not turn them into this lane's edits.

## Evidence and remaining work

Acceptance: target/effective-line-evidence/20260915-chatgpt-official-body-bounds/scope.json.
Observed at 2026-09-15T12:59:41.322Z; SHA-256:
4e2921786f11aedec2696b33b76527983f70c4bbced59ebf27bf8aa0a9f3c832.
Frozen input union: 1797; unchanged neighbors: 1791; unchanged web/Tauri assets: 22.
Original source, paired logs, format/gate receipts, snapshots and reviews remain
beside the scope. publication.json verifies final documentation, source/evidence
hashes, script limits and repository status without rerunning green native tests.

Strict remains expected exit 1: 2092 scanned, 12 hard, 20 mandatory and 40 soft.
32 files remain above 700: 20 Rust and 12 runtime-profile/vendor files. Clearance
stays 113/145 (77.9%). No checker, policy, baseline or exception was changed.
Gateway HEAD remains 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 190 modified,
one unstaged deletion, two staged deletions and 2293 untracked paths at scope.
This report adds one untracked path at publication. Neuro HEAD remains
bf818f0324024634bc890585efb78cc8e603d11a: 10 modified and 182 untracked paths.
No commit, service/profile mutation or release was performed.

The limit covers accumulated transport-decoded bytes, not allocator capacity,
JSON/charset expansion, CPU cost or aggregate concurrent process memory. Actual
allocator failure was not fault-injected. Large-body tests can consume more memory
if run concurrently outside this lane's --test-threads=1 runner. MIME matching,
synchronous solver scheduling, live-provider compatibility, remaining strict debt
and broader feature/language/packaged runtime/UI/Docker/release gates remain open.
S06 original Rust/Gemini implementation, cursor and final release-build coordination
stay reserved under GWP-20260912-01. Persistent target remains 4200, with no
persistent 4226. Overall optimization remains active.
