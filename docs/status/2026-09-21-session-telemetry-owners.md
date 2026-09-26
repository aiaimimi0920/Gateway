# Management session tests and telemetry ownership

Date: 2026-09-21. Status: structural_green; full goal remains active.

## Management-session tests

The original 638-effective-line test file is split by lifecycle responsibility.
All 14 original descriptions, bodies and assertions remain exactly once.

| File under `apps/desktop/src/session/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `ManagementSessionProvider.test.tsx` | 163 | Bootstrap, restore and interrupted-response recovery (4 tests) |
| `ManagementSessionProvider.host-origin.test.tsx` | 130 | Origin isolation and stale asynchronous results (4 tests) |
| `ManagementSessionProvider.credentials.test.tsx` | 80 | Grant/storage/rotation/logout lifecycle (4 tests) |
| `ManagementSessionProvider.secret-expiry.test.tsx` | 69 | Expiry and browser timer ceiling (2 tests) |
| `managementSessionTestFixtures.tsx` | 230 | Original API mock, session fixture, deferred factory and render harness |

Each suite retains the original local/session-storage beforeEach cleanup.
Fake-timer tests retain try/finally restoration; local APIs, adapters and
promises remain per-test. The authenticated session fixture is unchanged;
none of the suites mutates it. Vitest discovery, isolation, worker bounds and
global setup remain unchanged. No production session behavior was edited.

Evidence: `target/effective-line-evidence/20260921-management-session-tests/`.
The original suite passes 14/14; the four resulting suites pass 14/14. Desktop
typecheck passes. Exact source projection, UTF-8/no-BOM, independent review,
checker 19/19, ratchet and separate Gateway/Neuro diff checks pass. Strict at
this checkpoint reports 2235 scanned, 10 hard, 18 mandatory and 32 soft.

## Telemetry source ownership

`apps/desktop/src/features/console/telemetry.ts` decreases from 578 to 362
effective lines and retains snapshot construction, normalization and the
associated input/output types. `telemetryRollups.ts` (145) owns account/model
aggregation; `telemetryPresentation.ts` (73) owns status wording, billing ceiling
and relative time display. The three current consumers import their actual
owners directly, without a compatibility barrel. Their effective counts remain
81 (`useConsoleProviderMetrics.ts`), 421 (`accountLedgerSections.ts`) and 172
(`pilotStatsView.ts`).

All production function bodies are preserved exactly. The shared pure
laterTimestamp function becomes an export from the snapshot owner; rollups
depend on it, with no reverse dependency or new cycle. Null/zero distinctions,
account/model deduplication, model-level limiter attribution, quota traversal
depth and latest-failure association remain unchanged. No new I/O, timer,
queue, state mutation or allocation pass is introduced by moving the owners.
Chinese strings remain unchanged UTF-8 text.

Five new behavioral contracts were run against the original implementation
before extraction, then unchanged against the new owners. They cover shared
account deduplication, missing versus observed metrics, model concurrency and
duplicate targets, poll presence, and deterministic display semantics. The test
owner is 114 effective lines; these tests are not claimed as full snapshot or
UI/provider acceptance.

Evidence: `target/effective-line-evidence/20260921-telemetry-owners/`.

| Check | Result |
| --- | --- |
| Telemetry baseline and extracted-owner tests | 5 passed / 5 passed |
| Desktop typecheck before and after extraction | Passed / passed |
| `npm run build:web --prefix apps/desktop` | Passed |
| Exact source projection for the three production owners | Passed |
| UTF-8 without BOM for all seven touched telemetry source/test files | Passed |
| Independent source/consumer review | No extraction regression found |
| Checker tests / ratchet | 19 passed / passed |
| Separate Gateway and Neuro `git diff --check` | Passed |

The desktop package has no formatter script; existing formatting was retained.
No Rust source changed in this batch, so prior Rust/global-format results were
not re-run or promoted into a fresh full-gate claim. Both extraction evidence
directories retain baseline sources and `extraction-proof.json` hashes/counts.

## Limits and continuation

Review identified inherited assumptions to retain in the final audit: timestamp
ordering expects normalized values, model deduplication keys assume constrained
account IDs, input cost rows are assumed unique and inventory/state credential
IDs must share an identity scheme. These were not newly introduced or proven
defective by this move. Quota recursion, billing-group combinations and complete
snapshot folds are not covered by the five new tests. No fixes or exhaustive
coverage are claimed for those areas.

Across both extractions, strict inventory changes 2231 -> 2238 scanned and
33 -> 31 soft entries. Hard/mandatory counts stay 10/18; strict still exits 1
with 28 files above 700. No baseline, policy, exception or browser-profile
change was made. Existing dirty Gateway and Neuro submodule state are retained;
no sibling source, staging, commit, push or live service was changed.

Next unreserved desktop candidates are `useGatewayDesktopState.ts` (627),
`ModelPoolWorkspace.tsx` (624), `tools/publish-web-dist.mjs` (613) and
`ProviderCatalogDialog.tsx` (503). Revalidate ownership and exact boundaries
before editing. The full acceptance checklist stays active in
`2026-09-21-refactor-completion-audit.md`.

The web build is a local frontend artifact, not a packaged Gateway release.
No release package was placed in the requested release directory this batch;
S06 source/build transfer and runtime-artifact governance remain unresolved.
