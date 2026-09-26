# Full refactor completion audit

Date: 2026-09-21. Objective: finish the entire Gateway splitting/optimization
plan and verified release. Status: active, completion not proven.

Reconciliation checkpoint (2026-09-22): the historical counts and candidate
order below are retained as an audit snapshot. The authoritative current
inventory is now recorded in
`docs/status/2026-09-22-large-file-governance.md`: 2,335 files, 6 above 1,500,
8 in 701-1,500, and 11 in 501-700. The remaining >700 entries are the two
reserved S06 Rust hubs plus twelve immutable runtime payloads; no new source
ownership transfer has been recorded. Do not use the historical ModelPool or
28-entry candidate list below as a current ownership receipt.

This audit preserves the final conditions in sections 2.2, 9 and 12 of
`docs/plan/2026-09-03-gateway-effective-line-refactor.md`. A scoped green
checkpoint cannot satisfy the full objective. This document is not permission
to take over another writer's scope or migrate checker policy.

## Current evidence and unfinished requirements

| Requirement | Current evidence | Remaining work |
| --- | --- | --- |
| Strict closure for all included handwritten files | `target/effective-line-evidence/20260921-web-publisher-owners/strict-after.json`: 2243 scanned, 10 hard, 18 mandatory, 28 soft; strict exit 1 | Clear remaining 28 entries above 700, with runtime artifact governance separately classified |
| Genuine, current 501-700 exceptions | Four existing exception records match current strict-report source hashes/counts and have December 2026 review dates | Verify original independent approval evidence at final audit; resolve remaining 24 soft entries without invented approvals |
| Cohesive owners and preserved behavior | Keepalive now has 29 Rust owners, all <=481 effective lines; prior accepted lanes have their own reports | Complete remaining owners and audit their dependency/state/lifecycle boundaries; do not substitute line counts for architecture review |
| Paired request/stream/error/persistence proofs | Focused incremental logs exist; keepalive source projections preserve existing bodies with explicit probe control-flow adaptation | Complete each pending lane's paired proofs and final integrated suites |
| Full Python, Node, Rust, desktop/Tauri and provider line matrix | Incremental all-target Rust compilation, desktop typecheck and web build pass; the full matrix has not run | Run the actual full commands from main-plan section 9 and current CI after integration |
| Global formatting and release-candidate gates | Scoped keepalive formatting passes; inherited global formatting debt remains | Resolve owning-scope formatting, run full formatter and `tools/verify-gateway-release-candidate.ps1 -AsJson` |
| Immutable release in the user destination | No new package from the current structural batches; latest frontend web build is local only | Obtain shared build-window transfer, freeze integrated inputs, run official builder/packager, record manifest and checksums |
| Runtime/UI/integrity/Docker acceptance | Not established by unit tests or source moves | Validate the new package; preserve live services, clean temporary resources, verify final 4200-only persistent test stack and no persistent 4226 stack |
| Safety/performance/resource review | Incremental reports list unresolved lifecycle/body-bound risks | Finish relevant hardening and regression evidence in separate patches; disclose genuine residual risks |

The four matching exception paths are `BrowserConsoleApp.tsx`,
`useConsoleController.ts`, `src/protocol/accio/line/event_parse.rs` and
`src/protocol/responses/to_responses.rs`. Hash/date agreement verifies current
identity, not the historical approval itself. The remaining 24 soft entries
include two runtime extension files; the other 22 remain candidates for source
splitting or a justified independently approved exception.

## Remaining >700 Rust source scope

The latest report contains 16 Rust entries, all involving Gemini/upstream:

- `src/upstream/client.rs` (15638)
- `src/protocol/gemini_canvas.rs` (7784)
- `src/upstream/gemini_canvas_image_edit_local_helpers.rs` (2884)
- `src/upstream/gemini_canvas_direct_http_helpers.rs` (2044)
- `src/upstream/gemini/canvas_program_web_reverse/tests.rs` (1659)
- `src/console/gemini_auth_sessions.rs` (1648)
- `src/upstream/gemini/canvas_web_reverse/tests.rs` (1468)
- `src/protocol/gemini_business.rs` (1244)
- `src/upstream/gemini_canvas_music_helpers.rs` (1031)
- `src/upstream/gemini_canvas_official_api_helpers.rs` (943)
- `src/upstream/gemini/api/media.rs` (889)
- `src/upstream/gemini_canvas_request_headers.rs` (879)
- `src/protocol/gemini/web_reverse/response.rs` (819)
- `src/upstream/gemini/canvas_program_web_reverse/app_endpoint.rs` (756)
- `src/protocol/gemini/api/media.rs` (735)
- `src/upstream/gemini/canvas_web_reverse/result.rs` (726)

The board and `GWP-20260912-01` still retain the original S06 Rust/Gemini scope
and final shared build window. An explicit current source/build transfer was
requested in this goal turn. Neither an old status entry nor absence of a
process proves the former executor has relinquished ownership. Other unreserved
soft-limit and hardening work remains available while this is unresolved.

## Runtime artifact governance

Twelve >700 entries are browser-profile extension/WASM payloads. Git ignore and
absence from `git ls-files` do not exclude them from the line checker. The
current policy has no browser-profile exclusion. Their retained disposition and
outstanding source/license evidence are documented in
`docs/plan/runtime-profile-governance-proposal.md` and main-plan section 7.1.

Do not edit, delete or merge profiles, add a blanket exclusion, or regenerate
the baseline. A separate exact-path/hash provenance proposal, checker
regressions and explicit governance approval are required. Count any approved
artifact reclassification separately from actual source extraction.

## Next safe work

The keepalive gate passed 28/28 plus steward 3/3 and all-target compilation.
Console Redis storage is now 440/159/104 effective lines, with paired unit
tests 7/7 and external key contracts 4/4 passing. Continue the
unreserved soft-limit owners and
relevant hardening while waiting for S06 source/build transfer. Re-audit the
strict inventory and this complete checklist after integration; leave the goal
active until every final condition has fresh authoritative proof.

Console auth, request headers and rate-rule owners are now verified: parents
are 359/165/268 effective lines and all seven files are <=359. Paired tests
6/6, 13/13, 11/11 (two live Redis tests ignored), 2/2 and all-target compilation
pass; exact source and Lua proofs preserve existing behavior. Global formatting
still fails on the two reserved Gemini runtime-mirror files.

Routing candidate policy is now 429/119 effective lines, with paired 15/15 tests
and all-target compilation passing. Credential document test ownership is now
185/199/80/169 plus fixture 12, with paired 13/13 tests and desktop typecheck
passing. Both extractions preserve exact bodies and pass independent review.

ManagementSessionProvider tests are now 163/130/80/69 plus fixture 230, with
paired 14/14 tests and typecheck passing. Telemetry owners are 362/145/73, with
five paired behavioral contracts, desktop typecheck and web build passing.
Both extractions have source projection and independent review evidence.

Catalog directory is now 447/87; desktop profile/runtime owners are 421/265
plus notices 10. Paired catalog 2/2 and state 5/5 tests, typecheck and web build
pass. Saved-baseline review confirms callback/ref/invalidation and start/save
ordering. Existing async profile/refresh race and cancellation coverage remains
separate hardening work, not repaired by extraction.

Web publisher owners are now 407/180/30, with paired 13/13 real-process Python
contracts, syntax, typecheck and rsbuild publication passing. Exact source
projection passes, but independent review is pending after agent authentication
failure; complete that review before final acceptance.

The next unreserved desktop candidate identified by this audit,
`ModelPoolWorkspace.tsx (624)`, was completed in the coordinator continuation
recorded at `docs/status/2026-09-24-model-pool-workspace.md`. The current
worktree source snapshot used for that continuation measured 383 effective
lines before extraction; no reserved scope or API transfer was inferred.
