# Gemini Live refactor and compile recovery

This batch resumes session `01a0c33d-529c-7cb0-a564-080f68a0e56e`.
It completes the interrupted Gemini Live payload extraction and repairs the
Console, Cohere and Grok extraction boundaries that prevented the same Gateway
library from compiling. All implementation changes are inside Gateway.

Current state: all-target compilation, repository formatting, the size ratchet
and all 44 focused Rust tests pass. Source validation is complete for this batch.
A new immutable release was built and packaged after the resumed S06 build
window. Release validation is complete for the published package.

## Changes and effective lines

Counts use `scripts/effective-code-lines-lexer.mjs`, not physical lines. The
before column describes the interrupted working tree at the start of this run.

| File | Before | After | Responsibility |
| --- | ---: | ---: | --- |
| `src/http/routes/gemini_live.rs` | 501 | 453 | WebSocket and session orchestration |
| `src/http/routes/gemini_live/payload.rs` | 99 in misplaced sibling | 159 | Gemini Live request serialization |
| `src/http/routes/gemini_live/tests.rs` | Inline tests | 176 | Existing and additional wire-contract tests |
| `src/console/config.rs` | 187 | 201 | Console configuration and Redis namespace validation |
| `src/console/config/paths.rs` | 246 in misplaced sibling | 246 | Lexical and physical release-path containment |
| `src/protocol/cohere.rs` | 488 | 489 | Cohere normalization, streaming and public exports |
| `src/protocol/cohere/packing.rs` | 125 | 132 | Cohere request and success-response serialization |
| `src/protocol/grok/line.rs` | 492 | 454 | Grok response processing and adapter exports |
| `src/protocol/grok/line/packing.rs` | 84 in misplaced sibling | 103 | Grok payloads, model modes and request plans |

Every changed or extracted owner is below 500 effective lines. No exception or
baseline change is needed. The pre-extraction Gemini Live file at Git HEAD is
655 effective lines; the interrupted extraction had reduced it only to 501.
JSON macro layout is expanded for readability rather than compressed to lower
the measured line count.

The initial `cargo check --offline --locked --lib` exited 101 with nine errors:

- Three declared modules were placed beside their parent instead of in the
  parent's module directory: Console paths, Gemini Live payload and Grok packing.
- Console extraction omitted the Redis namespace validator and required parent
  access to the resolved path type and lexical path.
- Cohere normalization lost its tool-choice import, and its public packing
  functions had become private imports.
- Gemini Live setup lost access to its optional-value helper.

The Console namespace validator is restored verbatim from Git HEAD. Its
1-64 byte ASCII allowlist and error contract are preserved. Public Console,
Cohere and Grok entry paths remain available. Grok's unused duplicate text
combiner and forwarding wrappers are removed in favor of the packing owner.

Source comparison, ignoring indentation, blank lines and intended visibility
changes, preserves all 10 retained Gemini Live functions, all five extracted
payload functions and all 14 Console path functions. The restored Redis
validator also matches exactly without normalization.

## Verification

Evidence directory: `target/refactor-evidence/20260921-gemini-live-resume/`.

| Check | Result |
| --- | --- |
| Initial library compile | Failed with nine reproduced errors; `cargo-before.log` |
| `cargo check --offline --locked --all-targets` | Passed on final source; `cargo-all-targets-final.log` |
| `cargo fmt --all -- --check` | Passed for the whole Gateway repository |
| `npm run test:effective-lines --prefix scripts` | 19 passed, zero failed |
| `npm run check:effective-lines --prefix scripts` | Passed |
| Strict size audit | 28 existing violations; `effective-lines-after.json` |
| Gemini Live focused tests | 5 passed; `test-gemini-live.log` |
| Cohere focused tests | 2 passed; `test-cohere.log` |
| Grok focused tests | 10 passed; `test-grok.log` |
| Console Redis focused tests | 7 passed; `test-console-redis.log` |
| `console_config_contract` | 20 passed; `test-console-config.log` |
| Gateway and Neuro `git diff --check` | Passed separately |
| Source encoding | Changed Rust files are UTF-8 without BOM |
| Official release builder `-DryRun` | Passed; build command sequence verified |

All five Rust groups were run with `--offline --locked` and
`--test-threads=1`; none failed or was ignored. The final all-target check was
run after those tests on the final source. The Gateway and Neuro worktrees both
retain their pre-existing dirty state; sibling project changes are not accepted
as tested by Gateway's results.

The three additional Gemini Live tests cover explicit tool-config precedence,
null fallback with false/zero values, multimodal part order and system-history
exclusion, and tool-call IDs/order with malformed-argument fallback. The two
existing null-omission tests are retained.

The extraction introduces no new I/O, background task, subprocess, lock, retry
or resource owner. Existing socket/session lifetime and header/auth processing
remain unchanged. Pre-existing unbounded Gemini Live conversation history is
outside this behavior-preserving batch. Existing S06 compiler warnings remain
in image-edit and music helpers; no scoped warning is introduced.

## Remaining debt and release

The fresh strict audit scans 2,264 files:

- Above 1,500 effective lines: 10 files.
- Between 701 and 1,500: 18 files.
- Between 501 and 700: 20 files, down from 21 at resume.
- Total above 700: 28 files, unchanged by this soft-debt closure.

Of the 28 files above 700, 16 are Gateway Rust implementation/test files and
12 are browser-profile third-party/runtime files. The latter remain counted by
the current policy; this batch does not change policy or runtime profiles.

| Remaining Gateway source/test file | Effective lines |
| --- | ---: |
| `src/upstream/client.rs` | 15,638 |
| `src/protocol/gemini_canvas.rs` | 7,784 |
| `src/upstream/gemini_canvas_image_edit_local_helpers.rs` | 2,884 |
| `src/upstream/gemini_canvas_direct_http_helpers.rs` | 2,044 |
| `src/upstream/gemini/canvas_program_web_reverse/tests.rs` | 1,659 |
| `src/console/gemini_auth_sessions.rs` | 1,648 |
| `src/upstream/gemini/canvas_web_reverse/tests.rs` | 1,468 |
| `src/protocol/gemini_business.rs` | 1,244 |
| `src/upstream/gemini_canvas_music_helpers.rs` | 1,031 |
| `src/upstream/gemini_canvas_official_api_helpers.rs` | 943 |
| `src/upstream/gemini/api/media.rs` | 889 |
| `src/upstream/gemini_canvas_request_headers.rs` | 879 |
| `src/protocol/gemini/web_reverse/response.rs` | 819 |
| `src/upstream/gemini/canvas_program_web_reverse/app_endpoint.rs` | 756 |
| `src/protocol/gemini/api/media.rs` | 735 |
| `src/upstream/gemini/canvas_web_reverse/result.rs` | 726 |

The required release root remains
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
The currently existing immutable version is
`20260908-producer-mailbox-s06-123700`; no new package is claimed by this report.

[GWP-20260912-01](../plan/parallel-refactor-handoff.md#GWP-20260912-01-remaining-S06-ownership-and-final-build-coordination)
requires an explicit S06 source/docs freeze and build-window receipt, or user
confirmation that the original executor has stopped and the build scope may be
transferred. The user was asked for that confirmation after the compilation and
formatting gates passed. An idle native build directory alone does not satisfy
the recorded coordination rule.

After transfer and successful focused tests, freeze the source snapshot, run
`tools/build-gateway-release.ps1`, package a new version with `-SkipBuild`,
`-ReleaseRoot` set to the required root and `-AllowCustomReleaseRoot`, then run
the documented integrity, packaged runtime and desktop release smoke gates.
The broader large-file migration remains open.

## Resumed release validation

The resumed conversation treated the user's continuation request as transfer
of the stopped S06 executor's remaining build scope. A fresh process census
found no Gateway Cargo job (the concurrent `beaver-core` jobs belonged to a
sibling project). The official release builder then completed successfully.

| Check | Result |
| --- | --- |
| `tools/build-gateway-release.ps1` | Passed; headless Gateway and Tauri shell built |
| `tools/package-gateway-release.ps1 -VersionId 20260922-gemini-live-s06-closure -SkipBuild` | Passed; 1,368 files published |
| Packaged release root | `Neuro/release/Gateway/20260922-gemini-live-s06-closure` |
| Package integrity smoke | Passed; manifest SHA-256 `f100c59eceba0aa1cca3b4c76f91e33d6b8238d74b726c0b03e357f2724cb204` |
| Packaged runtime smoke | Passed; `/healthz`, `/readyz`, `/v1/models`, validation errors, 404, metrics, authorized drain and cleanup |
| Desktop artifact smoke | Passed; `gateway.exe` and `gateway-ui.exe` bytes/hash match manifest |
| Desktop launch smoke | Passed for a 3-second launch; no release-owned sidecar leaked |

The release artifacts report `gateway.exe` SHA-256
`002304687407bd382dbbef0e6ce6fb1bb280c33de8a74aceb7b5bd44c3edeaf2` and
`gateway-ui.exe` SHA-256
`27d6aa92de806f7d343c5e821c0e3e4ebf1d7fba1e2fa141514a319c13bd70a3`.
The broader large-file migration and live deployment remain outside this
closure.
