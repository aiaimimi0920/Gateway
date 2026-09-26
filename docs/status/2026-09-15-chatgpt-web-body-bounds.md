# ChatGPT Web response byte admission, 2026-09-15

Six ChatGPT Web reverse whole-body reads now reuse the existing 64 MiB
provider-aware charset/BOM collector: bootstrap, requirements, prepare,
accumulated conversation, streaming non-success and successful non-SSE diagnostics.
Declared overflow is rejected before body collection. Unknown-length overflow is
rejected during collection without waiting for EOF, with the fixed operation label,
provider chatgpt_web_reverse, ServerError/HTTP 500 and upstream_body_too_large code.
Successful SSE still uses bytes_stream and the existing incremental translator.

Only four production files change: bootstrap 256 -> 259, execution 184 -> 191,
requirements 158 -> 161 and transport 167 -> 170 effective lines. The test entry
adds three module declarations, 419 -> 422. New body_wire/body_targets/body_limits
owners measure 130/100/171. Every modified owner remains below 500. Request order,
headers, timeout floors, classifiers, parsers and browser fallback are unchanged.
The shared reader, official_api, policies, dependencies and profiles are unchanged.

## Verification

Accepted baseline and candidate use the same frozen tests and serialized commands:

    cargo test --offline --locked --lib chatgpt:: -- --test-threads=1
    cargo test --offline --locked --lib protocol::upstream_body:: -- --test-threads=1

Both phases use GATEWAY_PREBUILT_WEB_UI=1. ChatGPT baseline: 98 passed / 12 failed;
candidate: 110/110, zero failed/ignored, 2932 filtered out. The twelve baseline
failures are precisely the six boundaries times declared/chunked overflow, each
unable to produce rejection before EOF. All previous 95 ChatGPT identities and
three preservation tests pass in both phases. Shared reader tests pass 12/12 in
both phases, with matching identities and warnings. They cover exact/over limit,
chunk order, provider errors, charset/BOM, cancellation/drop and request deadlines.

New tests invoke actual production boundaries through ephemeral no-proxy loopback
HTTP. Oversized responses remain open, and chunked overflow omits the terminal
chunk. Fixtures use a fixed 64 KiB write buffer, bounded request headers and owned
task abort/await cleanup. Preservation cases admit exactly 64 MiB, decode an actual
windows-1252 conversation, and receive Paris before the live SSE upstream is
released to send its terminal frame. Exactly one translated DONE is observed.
Fixtures do not separately acknowledge server-observed client disconnect.

Gateway default cargo check --offline --locked --all-targets, scoped official
rustfmt --check, checker tests 19/19 and ratchet pass. Both Gateway and Neuro pass
staged/unstaged Git diff checks. Gates were serialized against frozen source/tests/
assets; all native sessions are terminal and process guards are idle. Independent source
and fixture reviews found no introduced defect within their inspected scope.

The first evidence projection assumed LF and failed on inherited bootstrap CRLF
before starting Cargo. The comparison now normalizes line endings; actual frozen
file hashes are unchanged. projection-failure.json preserves that failed evidence
attempt. The exact source proof establishes six replacements plus required import
adjustments after CRLF-to-LF normalization. Existing test bodies remain unchanged.

## Evidence and remaining work

Acceptance: target/effective-line-evidence/20260915-chatgpt-web-body-bounds/scope.json.
Observed at 2026-09-15T12:22:03.227Z; SHA-256:
3d5a75a738f3ded7eedeb535d36784e8b42514a6c0a6c01528b14310fbe4bd68.
Frozen input union: 1792; unchanged neighbors: 1784; unchanged web/Tauri assets: 22.
Original source, baseline/candidate logs, receipts, snapshots, failed projection
and review remain beside the scope. publication.json verifies final documents,
source and evidence hashes, script limits, and exact repository status.

Strict remains expected exit 1: 2087 scanned, 12 hard, 20 mandatory and 40 soft.
32 files remain above 700: 20 Rust and 12 runtime-profile/vendor files. Clearance
stays 113/145 (77.9%); this repair claims no additional large-file clearance.
Gateway HEAD remains 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 189 modified,
one unstaged deletion, two staged deletions and 2286 untracked paths at scope.
This report adds one untracked path at publication. Neuro HEAD remains
bf818f0324024634bc890585efb78cc8e603d11a: 10 modified and 182 untracked paths.
All inherited repository state remains preserved; no commit or release occurred.

The 64 MiB contract limits accumulated transport-decoded response bytes. Charset
decoding still needs additional allocation and CPU, and there is no aggregate
concurrency or total decoded/process RSS budget. Successful SSE has no new
aggregate body cap. Official API body reads, MIME matching, synchronous solver
scheduling and live-provider compatibility remain separate follow-up work.
Broader strict/feature/language/provider/packaged runtime/UI/Docker/release gates
remain open. S06 original Rust/Gemini implementation, cursor and final release
build coordination stay reserved under GWP-20260912-01. Persistent target remains
4200, with no persistent 4226. Overall optimization remains active.
