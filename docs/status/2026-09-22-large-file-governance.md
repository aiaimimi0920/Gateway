# Large-file governance checkpoint

This checkpoint continues the Gateway large-file migration after the Gemini Live
release closure. The effective-line checker is authoritative; counts below are
from the current working tree after the Gemini helper extraction batches.

Follow-up: [Gemini helper and media owners](2026-09-22-gemini-helper-owners.md)
records the next four completed splits and the updated remaining inventory.

Follow-up: [Gemini auth session owners](2026-09-22-gemini-auth-session-owners.md)
and [Gemini Business test owners](2026-09-22-gemini-business-test-owners.md)
record two additional completed splits from that inventory.

Follow-up: [Gemini image-edit local owners](2026-09-22-gemini-image-edit-local-owners.md)
records the signaler, transport, and focused test owners completed in this batch.

Follow-up: [Gemini direct HTTP owners](2026-09-22-gemini-direct-http-owners.md)
records the direct HTTP contract, error, page-harvest, JSON, and test owners
completed in the next batch.

Follow-up: [S06 hub inventory](2026-09-22-s06-hub-inventory.md) records the
reserved `client.rs` and `gemini_canvas.rs` boundaries without taking over the
existing executor's Rust/Gemini scope.

Follow-up: [packaged runtime smoke owners](2026-09-22-packaged-runtime-smoke-owner.md)
records the integrity and runtime owners extracted from the packaged smoke entry
point.

Follow-up: [release-candidate gate owner](2026-09-22-release-candidate-owner.md)
records the runtime lifecycle owner extracted from the release-candidate gate
entry point.

Follow-up: [Udio capture utility owner](2026-09-22-udio-capture-owner.md)
records the input and browser owners extracted from the Udio capture entry point.

## Inventory at this checkpoint

The repository scan covers 2,335 files:

| Classification | Count | Meaning |
| --- | ---: | --- |
| Above 1,500 effective lines | 6 | Hard violations; no exception path |
| 701-1,500 effective lines | 8 | Mandatory migration debt |
| 501-700 effective lines | 11 | Soft-limit review or independent exception |
| Above 700 total | 14 | Remaining files not yet below the migration ceiling |
| Above 500 total | 25 | Remaining files requiring governance attention |

The 14 files above 700 comprise 2 handwritten Gateway Rust files and 12
browser-profile/runtime payload files. The Rust debt is the active split target;
the browser payloads require a separate provenance and policy decision and must
not be “fixed” by deleting, minifying, or blanket-excluding them.

## Completed in this checkpoint

`src/upstream/gemini_canvas_official_api_helpers.rs` was split by responsibility:

- parent helper: 943 -> 500 effective lines;
- `.../tests.rs`: 278 effective lines, containing the nine focused tests;
- `.../video.rs`: 173 effective lines, containing official video polling and
  download orchestration.

The public `execute_gemini_canvas_official_video` signature and call path remain
unchanged. The stale exceptions for `src/protocol/accio/line/event_parse.rs` and
`src/protocol/responses/to_responses.rs` were removed because both are now below
the soft threshold. The strict checker now reaches the actual remaining debt.

The second extraction split `src/upstream/gemini/canvas_program_web_reverse/tests.rs`
from 1,659 effective lines into a 103-line registration owner plus four focused
test owners:

- `tests/payload_contracts.rs`: 363 effective lines;
- `tests/runtime_contracts.rs`: 409 effective lines;
- `tests/response_contracts.rs`: 369 effective lines;
- `tests/endpoint_contracts.rs`: 423 effective lines.

All 47 original tests remain present and pass. The former hard violation is now
five cohesive test modules, each below 500 effective lines.

The third extraction split `src/upstream/gemini/canvas_web_reverse/tests.rs`
from 1,468 effective lines into a 128-line registration owner and four focused
test owners:

- `tests/input_contracts.rs`: 288 effective lines;
- `tests/response_contracts.rs`: 492 effective lines;
- `tests/audio_contracts.rs`: 134 effective lines;
- `tests/media_result_contracts.rs`: 434 effective lines.

All 63 browser-owned canvas tests remain present and pass. The former mandatory
test file is now below the migration ceiling with no exception record.

The current extraction split
`src/upstream/gemini_canvas_image_edit_local_helpers.rs` from its large inline
test and signaler owners. The parent now keeps a narrow facade while the
signaler lifecycle, HTTP transport, and seven focused test/support owners are
all below 500 effective lines. All 74 image-edit local-helper tests remain
present and pass.

The following extraction split
`src/upstream/gemini_canvas_direct_http_helpers.rs` into contract, error,
page-harvest, JSON transport, and six focused test/support owners. The facade
preserves the existing import path; all new owners are below 500 effective
lines. All 55 direct HTTP helper tests remain present and pass.

The release-candidate gate split keeps the public verifier entry point at 267
effective lines and moves runtime smoke transport, assertions, process
lifecycle, and environment restoration into a 292-line owner. Its gate order,
canary safety checks, JSON contract, and cleanup behavior remain unchanged.

The Udio capture utility split keeps browser connection and request lifecycle in
the 228-line entry point while moving input/file helpers and CDP/page hooks into
107-line and 231-line owners. The stdin/stdout protocol and browser cleanup
remain unchanged.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo check --offline --locked --lib`: passed.
- Focused official API tests: 9 passed, 0 failed.
- Focused canvas-program web-reverse tests: 47 passed, 0 failed.
- Focused canvas web-reverse tests: 63 passed, 0 failed.
- Focused image-edit local-helper tests: 74 passed, 0 failed.
- Focused direct HTTP helper tests: 55 passed, 0 failed.
- `cargo check --offline --locked --all-targets`: passed.
- `npm run check:effective-lines --prefix scripts`: ratchet passed with every
  new Gemini, packaged-runtime, release-candidate, and Udio capture owner below
  500 effective lines.
- `npm run strict:effective-lines --prefix scripts`: inventory completed; it
  remains red only for the listed historical >700 files.
- `git diff --check`: passed.

## Next ownership order

The low-risk Gemini test/helper owners, `src/console/gemini_auth_sessions.rs`,
`src/protocol/gemini_business.rs`, the packaged-runtime smoke owners, the
release-candidate gate owner, and the Udio capture owner are now complete. The
current strict inventory is 2,335 files with 6 hard violations (>1,500), 8
mandatory entries (701-1,500), and 11 soft-limit entries (501-700).
The remaining Rust owners are
the two reserved S06 hubs (`src/upstream/client.rs` and
`src/protocol/gemini_canvas.rs`); the
browser-profile/runtime payloads still need a separate provenance/policy
decision. S06 hubs must be handled as separate batches rather than by parallel
edits to the same call graph.

## Current ownership audit

The post-extraction audit keeps the strict result at 2,335 scanned files:
6 files above 1,500 effective lines, 8 files in the 701-1,500 tier, and
11 files in the 501-700 tier. The 14 files above 700 remain exactly the two
reserved S06 Rust hubs plus the twelve browser-profile/runtime payloads. No new
source ownership transfer was found in the board or handoff records.

The remaining soft-limit entries have the following dispositions:

- `apps/desktop/src/features/console/BrowserConsoleApp.tsx` (549) and
  `useConsoleController.ts` (698) retain the approved source-hash-bound
  composition exceptions recorded by the Browser Console lane.
- `src/upstream/gemini/canvas_program_web_reverse/result.rs` (666),
  `src/protocol/gemini/canvas_web_reverse/stream_parsers.rs` (636),
  `src/upstream/gemini/canvas_web_reverse/execution.rs` (540), and
  `src/upstream/gemini_canvas_runtime_helpers.rs` (527) remain in the
  Rust/Gemini/S06 neighbourhood and are not available for takeover.
- `src/upstream/aistudio/web_reverse/execution.rs` (655) remains with the
  AI Studio worker/probe lane. `src/upstream/browser_worker_types.rs` (651)
  remains coupled to browser-worker/runtime ownership, and
  `tests/python/test_gateway_splitter_worker_e2e.py` (562) remains with the
  splitter/browser test lane.
- The two `deploy/gateway_data/browser-profiles/**/craw_window.js` entries
  (551 each) remain immutable runtime payloads pending the separate
  provenance/license policy decision.

Accordingly, this checkpoint makes no new source edit. A future split of a
Gemini, browser-worker, or splitter entry requires an explicit ownership
receipt and a focused boundary/test plan first. The strict audit remains active
until the S06 executor returns its Rust/Gemini scope and the runtime payload
governance decision is approved; the ratchet and checker tests remain green.

## Post-audit verification

- `npm run test:effective-lines --prefix scripts`: 19/19 passed.
- `npm run check:effective-lines --prefix scripts`: ratchet passed at
  2,335 scanned files.
- Fresh `npm run strict:effective-lines --prefix scripts` reports the same
  2,335-file inventory (6 above 1,500, 8 in 701-1,500, 11 in 501-700) and
  exits 1 only for the two reserved Rust hubs and the twelve immutable runtime
  payloads listed above.
- Udio capture, manual-browser, and browser-worker regressions: 31/31 passed.
- Release-candidate contracts: 23 passed with 14 subtests; packaged-runtime
  focused contracts: 23 passed with 7 subtests.
- `test_gateway_docs_consistency.py`: 4 passed with 3 subtests.
- The safe release-candidate invocation passed with manifest and canary
  preflight enabled; Python, line-matrix, Rust, release-build, browser-worker,
  Docker, and runtime steps were intentionally skipped.
- The broader standalone CI contract initially failed because
  `test_gateway_standalone_ci.py` still read only `build.rs` after the accepted
  build-time UI extraction moved those markers into `build_support/*.rs`.

## Build-time UI contract alignment

The stale standalone-CI contract was repaired without changing the build
implementation: it now reads `build.rs` together with the sorted
`build_support/*.rs` owners before asserting the existing Docker/prebuilt-UI
markers. This keeps the contract coupled to the complete extracted build
surface instead of requiring copied marker strings in the 153-line facade.

Fresh verification after that alignment:

- `test_gateway_standalone_ci.py`: 12 passed with 14 subtests.
- Combined release, package, standalone and build-time UI contracts: 58 passed
  with 35 subtests.
- Full `tests/python/test_gateway_standalone*.py` discovery: 59 passed with
  37 subtests.
- The CI-shaped `python -m unittest discover -s tests/python -p
  "test_gateway_standalone*.py" -q` run completed 59 tests with `OK`.
- Focused Rust build-time UI contracts also passed: `prebuilt_web_ui_build_contract`
  17/17 and `console_ui_assets_contract` 6/6, both with locked offline Cargo
  settings and single-threaded test execution.
- No production build source, Dockerfile, policy, baseline or release artifact
  was changed by this repair.

## Coordinator gate continuation

The coordinator rechecked the two preserved release directories without
rebuilding or replacing them. `20260908-producer-mailbox-s06-123700` passed
package integrity with manifest SHA-256
`2850dd49cd9a2a1fdcec288b2ed664920debecb8f0e4afcc2202b49d8d5b44e0` and
checksums SHA-256
`7732d1959498f9205099c8c68eed1d6714ea0bc383734b11639dc1e8065f7ae6`.
`20260922-gemini-live-s06-closure` also passed with manifest SHA-256
`f100c59eceba0aa1cca3b4c76f91e33d6b8238d74b726c0b03e357f2724cb204` and
checksums SHA-256
`70826a21728d7af3cd8c08a9b77422cee5a49d8c3cc6f7f878950d8d1109721b`.

The focused package/release contracts passed 30/30 with `PYTHONPATH=tests/python`.
The safe release-candidate invocation also passed: manifest validation and the
safe live-provider canary preflight passed, while Python, line-matrix, Rust,
release-build, browser-worker, Docker, and runtime-smoke steps were explicitly
skipped. This is evidence for preserved-package integrity and gate wiring only;
it does not satisfy the full release/runtime acceptance requirement.
