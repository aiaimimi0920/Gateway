# Gemini helper and media owner checkpoint

This batch continues the [large-file governance checkpoint](2026-09-22-large-file-governance.md).
Four existing Gemini source files became 15 cohesive source/test owners. All
results are below 500 effective lines; the largest has 415. This batch is
complete, while repository-wide large-file migration remains open.

## Ownership and line counts

Counts use the repository's effective-line lexer. The before values come from
working-tree snapshots taken before this batch, including inherited changes.

| Owner | Before | After |
| --- | ---: | ---: |
| `src/upstream/gemini_canvas_request_headers.rs` | 879 | 415 |
| `src/upstream/gemini_canvas_request_headers/cookies.rs` | New | 105 |
| `src/upstream/gemini_canvas_request_headers/tests.rs` | New | 363 |
| `src/upstream/gemini_canvas_music_helpers.rs` | 1,031 | 233 |
| `src/upstream/gemini_canvas_music_helpers/websocket.rs` | New | 209 |
| `src/upstream/gemini_canvas_music_helpers/tests.rs` | New | 349 |
| `src/upstream/gemini_canvas_music_helpers/tests/websocket.rs` | New | 257 |
| `src/upstream/gemini/api/media.rs` | 889 | 152 |
| `src/upstream/gemini/api/media/transport.rs` | New | 120 |
| `src/upstream/gemini/api/media/music.rs` | New | 367 |
| `src/upstream/gemini/api/media/music/frames.rs` | New | 153 |
| `src/upstream/gemini/api/media/video.rs` | New | 163 |
| `src/protocol/gemini/api/media.rs` | 735 | 410 |
| `src/protocol/gemini/api/media/audio.rs` | New | 179 |
| `src/protocol/gemini/api/media/responses.rs` | New | 165 |

The header owner retains profile/replay assembly; cookie application and ordered
response-cookie merging have their own owner. Canvas music separates accepted
results and follow-up contracts from WebSocket frames, decoding, and errors.
Official media execution separates dispatch, HTTP transport, music connection
lifetime, frame processing, and video polling/download. Protocol media separates
request normalization from audio decoding/formatting and response envelopes.
Existing callable helper and execution paths remain available through explicit
re-exports where needed.

## Preservation and regression evidence

Source projection matched all 154 original function definitions, including all
51 original tests. It found no missing functions or signature differences after
accounting for intended module-visibility changes. One removed trailing argument
comma is formatter-only. The only function-body behavior change is the UTF-8
preview correction below. The proof includes current SHA-256 values and confirms
UTF-8 without BOM for all 15 result files.

`compact_response_preview` previously sliced a normalized string at an arbitrary
byte offset, which could panic inside a non-ASCII character. It now moves that
offset back to a UTF-8 boundary, retaining the byte budget, suffix, normalization,
and ASCII behavior. The standalone reproduction against the original function
had 1 passing and 1 failing test; the corrected function passes both. Two focused
regressions now run in the real media test module, covering multibyte boundaries,
zero/exact limits, normalization, and the existing ASCII contract.

The per-owner review covered imports/visibility, header and cookie handling,
error contracts, HTTP/WebSocket ownership, polling, and cleanup paths. Existing
setup-handshake waiting and aggregate audio/body buffering retain their pre-batch
behavior; this extraction adds no broader lifecycle hardening.

## Verification

The same test filters ran before and after extraction using
`cargo test --offline --locked --lib FILTER -- --test-threads=1`:

| Filter | Before | After |
| --- | ---: | ---: |
| `upstream::browser_executor_helpers` | 21 passed | 21 passed |
| `gemini_canvas_request_headers` | 10 passed | 10 passed |
| `gemini_canvas_music_helpers` | 36 passed | 36 passed |
| `gemini::api::media` | 5 passed | 7 passed |
| Total | 72 passed | 74 passed |

The browser-executor results also close the earlier focused compile/test
uncertainty for that extraction. Final checks for this batch:

- `cargo check --offline --locked --all-targets`: passed after the last header
  import cleanup; final run finished in 2m 37s.
- `cargo fmt --all -- --check`: passed.
- `npm run test:effective-lines --prefix scripts`: 19 passed, 0 failed.
- `npm run check:effective-lines --prefix scripts`: passed.
- Strict effective-line scan: completed with exit 1 for the 21 remaining files
  above 700; none of the 15 result files needs an exception.
- Neuro `test-development-standard-contract.ps1`: passed for all six submodules.
- `git diff --check`: passed separately in Gateway and Neuro.

The two read-only subagent attempts returned HTTP 503. The evidence above records
primary-agent validation; no independent agent review was obtained.

Evidence is under
`target/refactor-evidence/20260922-gemini-helper-owners/`: `before/`,
`before-*.log`, `after-*.log`, `cargo-all-targets-final.log`,
`rustfmt-final.log`, `effective-lines-final.log`, `strict-after.json`,
`strict-final.log`, `development-standard-contract.log`,
`gateway-diff-check-final.log`, `neuro-diff-check-final.log`,
`preview_before.rs`, `preview_after.rs`, and `source-projection.json`.
The initial inventory is
`target/effective-line-evidence/20260922-continuation/before.json`.

## Remaining inventory and repository scope

The strict scan now covers 2,292 files. Files above 700 decreased from 25 to 21:
9 Rust owners and 12 browser-profile/runtime payloads. The classification is
9 hard violations, 12 mandatory migrations, and 14 soft-limit entries, giving
35 files above 500. The initial inventory was 13 Rust owners plus 12 runtime
payloads, with 39 files above 500; the preceding checkpoint's arithmetic and
classification have been corrected accordingly.

| Remaining Rust owner | Effective lines |
| --- | ---: |
| `src/upstream/client.rs` | 15,638 |
| `src/protocol/gemini_canvas.rs` | 7,784 |
| `src/upstream/gemini_canvas_image_edit_local_helpers.rs` | 2,884 |
| `src/upstream/gemini_canvas_direct_http_helpers.rs` | 2,044 |
| `src/console/gemini_auth_sessions.rs` | 1,648 |
| `src/protocol/gemini_business.rs` | 1,244 |
| `src/protocol/gemini/web_reverse/response.rs` | 819 |
| `src/upstream/gemini/canvas_program_web_reverse/app_endpoint.rs` | 756 |
| `src/upstream/gemini/canvas_web_reverse/result.rs` | 726 |

All remaining over-500 files outside this batch retain their initial inventory
hashes. In particular, the two reserved S06 hubs, `client.rs` and
`protocol/gemini_canvas.rs`, remain separate work. Runtime payload provenance
still requires its own policy decision; checker rules, baselines, and exceptions
were unchanged in this batch.

Gateway remains a dirty, uncommitted working tree with inherited work preserved.
This batch owns four modified source parents, 11 new source/test files, this
report, and the preceding checkpoint's correction/link. Neuro received no file
edits from this batch; its inherited root/submodule changes remain present.
Other subprojects received no edits or validation claims. No release was built
or deployed for this atomic extraction batch.
