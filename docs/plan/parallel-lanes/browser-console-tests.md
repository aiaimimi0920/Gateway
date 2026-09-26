# Lane U: browser console test ownership

Coordinator scope: BrowserConsoleApp.test.tsx and cohesive test-only owners in
apps/desktop/src/features/console. Production UI, external Rust/Gemini work and
shared build ownership are unchanged.

The current 49-test baseline has 27 passes and 22 failures, recorded in
target/effective-line-evidence/operations-workspace/codex-console-after.json.
Two obsolete Codex demo assertions were already repaired against real route
credentials. Other failures require current autosave, group-editor and telemetry
contracts, not restoration of removed product controls or synthetic data.

Structural extraction preserves every test body and fixture function first.
Split API stubs from rendering/navigation fixtures and move shell scenarios to
their own suite. Keep per-suite localStorage reset and normal Vitest cleanup.
Remaining large-file debt is not counted as cleared until all owners comply.
No new behavior or dependency is introduced by this batch.

## Fixture and shell extraction

Entry remains 2650 effective lines; shell suite 199, API fixture 176 and render
fixture 125. This intermediate batch earns no completed-debt credit. Exact AST
comparison preserves all 49 test bodies, every shared function and both copies
of the per-suite reset. Tests remain 27 pass / 22 fail with identical failed-name
sets; the extracted shell is 7/7. Typecheck, checker 19/19, ratchet and diff pass.
Evidence lives in target/effective-line-evidence/browser-console-tests:
before.tsx, verify-extraction.mjs, after.json, typecheck.log and ratchet.log.

The new modules are test-only leaves with no production import or new resource
ownership. Each API factory creates independent mocks; rendering retains the
existing provider and cleanup lifecycle. Synthetic test credentials stay fixture
data, not production seed accounts. All four files are UTF-8 without BOM.

## Complete structural decomposition

The coordinator replaced an unsuitable agent draft that duplicated scaffolding
into numbered single-test files and repeated a shell test. None of those 42
numbered files remain. The accepted structure groups scenarios by responsibility
and imports shared fixtures, with no duplicate test execution or fixture bodies.

Effective lines: main provider suite 208; shell 199; pool 377; Codex actions 372;
Codex library 430; account inventory/CRUD 427; groups 237; draft lifecycle 242;
Gemini Canvas 371; Gemini Business 148; API fixture 176; render fixture 125.
All 12 owners remain below 500 and UTF-8 without BOM. Runtime dependencies point
from suites to test fixtures and product entry; no product code imports tests.

Fresh AST proof preserves all original 49 test bodies, shared function bodies
and every suite reset. Direct TypeScript checking, ratchet and diff pass. The
BrowserConsoleApp filter executes 50 tests including the existing, untouched
i18n test: 28 pass / 22 fail. The original 49 remain 27 pass / 22 fail with
identical failed-name sets. Canvas 2/2 and shell 7/7 pass independently within
that run. No previously failing assertion was deleted or weakened in the split.

Evidence: final-split-tests.json, final-split-typecheck.log,
final-split-ratchet.log and verify-extraction.mjs. The checker 19/19 run from the
first batch remains applicable; checker implementation is unchanged. Inventory
is 1218 scanned, 38 hard / 69 mandatory / 38 soft. Original structural clearance
is now 38/145 (26.2%), with 107 files above 700. These counts do not establish
full product readiness; 22 console failures, wider hardening and native release
acceptance remain open. No Cargo/native build or release ran.

An independent autosave scout correctly identified static commit/refresh fixture
data loss. Its claim that a manual validation control remains valid was not
established by a rendered call site and conflicts with current failing selectors;
the coordinator has not accepted or implemented that suggestion.

## Current-contract repair: autosave and account data

Four single-commit scenarios now await the real 1200 ms autosave instead of
clicking a removed manual-save button: custom provider creation, credential add,
credential edit and Gemini Business import. All existing request assertions are
retained, including stable IDs, routing groups, model aggregation, secret-patch
paths/operations/values and the secret-access grant. A bounded 3000 ms wait only
observes the actual commit mock. These cases do not require a subsequent edited
draft, so the wider stateful API-fixture change is deliberately still pending.

LongCat coverage now requires exactly its configured account, unavailable numeric
telemetry and no invented success strip or demo pagination. The product omits
the success strip when it has no dispatch windows. Codex card-layout coverage
now supplies three explicit credential fixtures instead of expecting synthetic
accounts from an empty provider. No product implementation changed.

Fresh BrowserConsoleApp-filter result: 34 pass / 16 fail / 50 total (including
the existing i18n case). Six formerly failing scenarios now pass. Typecheck,
ratchet and diff pass. Evidence: contract-repair-tests.json,
contract-repair-typecheck.log, contract-repair-ratchet.log; earlier focused runs
are autosave-single-tests.json and real-card-tests.json. The latter captured an
incorrect intermediate expectation for an empty success strip; the actual
conditional rendering was inspected and the assertion corrected before the
final run. The pre-repair AST extraction proof is now historical, not a claim
that updated test contracts still equal the original assertions. Structural
clearance remains 38/145; native release and the 16 console failures remain open.

## Credential-pool contract checkpoint

The pool suite is now 4/4. The policy-edit scenario supplies an actual configured
automation driver before enabling prune; the independent no-driver scenario
still proves refill works while prune remains blocked. Both assert the document
submitted by autosave, using a bounded committed-draft observation helper, rather
than the removed manual validation button. The helper optionally accepts a prior
commit count for future multi-edit scenarios; it does not fake a save or call any
API itself. The obsolete split-text availability-count assertion was removed from
the policy test, whose target capacity and policy-switch assertions remain.

Typecheck, ratchet and diff pass. Evidence: pool-contract-tests.json,
pool-contract-typecheck.log and pool-contract-ratchet.log. Two failures from the
last full console run are resolved by this focused run; the entire console filter
was not rerun without further changes. Production and API fixture state are
unchanged. Native release remains pending.

Next statistics repair has a confirmed different data boundary: the stats dialog
uses polled consoleTelemetry (BrowserConsoleApp.tsx around 2875), and opening it
only updates dialog state (around 4274). Its stale test expects getCredentialUsage
to run, but current aggregates come from cost overview, audit summary and model
health. Repair the telemetry fixture and assert real account-level semantics;
do not add an extra production request to satisfy the old assertion.

## Statistics data-boundary checkpoint

The statistics scenario now provides typed cost-overview, request-audit and
credential-model-health responses. It verifies 32 billed requests, 28 completed,
4 failed, 1000 tokens, and an 87.5 percent decided-request success rate even when
the audit also contains 5 cancelled and 3 running requests. Credential health
separately shows one served model and 2 historical credential failures, distinct
from the provider's 4 audited failures. The account-aggregate heading remains
required, and getCredentialUsage must not be called.

The prior latency assertion belonged to the removed usage-bucket view. The new
assertions follow the rendered provider/credential ownership boundaries. During
fixture validation, the model name was found in the served-model title rather
than text, and the valid health status is active rather than healthy; both were
corrected after inspecting actual rendering and status mapping. No production
behavior was changed. Focused statistics test 1/1, typecheck, ratchet and diff pass.
Evidence: stats-contract-tests.json, stats-typecheck.log and stats-ratchet.log.
The full console filter has not been redundantly repeated. Remaining multi-edit
autosave, group navigation and draft-lifecycle failures still require repair.

## Autosave round trips and group controls

Codex action/library suites now pass 13/13. Routing metadata and dispatch tests
retain their actual group/switch assertions while dropping obsolete provider
summary text and rejecting stale preview timestamps/traffic. Scheduled probes
are checked through actual commit requests. A test-only, opt-in document fixture
keeps the committed route document and revision available to refresh; it does
not simulate secret storage. The two-step schedule/duplicate scenario captures
the previous commit count and proves the second document retains the original
credential's 15-minute schedule as well as the new account.

Group suite now passes 4/4. The coordinator corrected an incomplete agent draft:
replaced ambiguous indexed button selection with named expandable card controls,
removed a condition copied from an unrelated group scenario, narrowed mock access
with vi.mocked, and replaced remaining obsolete save buttons. Tests preserve
account migration from group A to B, new-group membership/name/billing fields,
and invalid-ID blocking. The invalid-ID test now waits beyond the autosave delay,
proves no commit, then fills the ID and verifies the resulting commit.

Fresh console-filter gate: 45 pass / 5 fail / 50 total, with all remaining failures
in BrowserConsoleApp.draft.test.tsx. Typecheck, ratchet and diff pass. Evidence:
actions-groups-all.json, actions-groups-typecheck.log and actions-groups-ratchet.log;
focused evidence includes codex-actions-tests.json and groups-final-tests.json.
No product changes, Cargo build or native release ran; all sessions terminal.

Read-only draft audit confirms handleValidate has no rendered invocation. Its
remaining tests must cover reachable autosave/error and refresh contracts rather
than manufacturing access to the dead handler. Group ID/billing editors are
available only after expanding the selected group card. This is the next repair.

## Desktop test gate closure

All remaining draft scenarios were reconciled with reachable controls. Group
editors are explicitly expanded. Same-revision refresh replaces the draft and
cancels its pending autosave; the test waits beyond 1200 ms and proves no stale
commit. The beforeunload test retains clean/dirty cancellation checks. Repair
feedback now comes from authoritative route diagnostics: successful refresh
clears it and failed refresh preserves it. These tests do not claim to exercise
the unreachable manual-validation state. Invalid billing blocks autosave across
the debounce interval, and valid scientific notation commits 0.01. The unused
manual-validation test helper was removed after confirming no callers remain.

The first full desktop run produced 241 passes and four 5-second timeout failures
in previously passing long console scenarios. Splitting suites increased concurrent
jsdom console instances. The desktop Vitest config now caps workers at four rather
than launching a console per available worker. No test timeout or assertion was
relaxed. The fresh normal-config full run passes all 245 tests across 47 files.
Typecheck, ratchet and diff pass; config and changed source remain UTF-8 without
BOM. Evidence: desktop-bounded-tests.json, desktop-final-typecheck.log and
desktop-bounded-ratchet.log. desktop-final-tests.json preserves the earlier
parallel-timeout failure for diagnosis.

This closes the offline desktop test gate for the current coordinator changes,
not the full optimization goal or native release. Structural clearance remains
38/145 with 107 files above 700. Cargo ownership transfer, broader runtime and
provider verification, remaining large-file work and the new immutable release
remain outstanding. All owned test sessions are terminal.

## Codex library ownership continuation

The 465-effective-line Codex library snapshot was split by behavior after the
earlier suite decomposition had added statistics and scheduled-probe coverage.
The statistics/scheduled-probe owner is 221 effective lines with two tests; the
rendering and legacy-control owner is 261 effective lines with six tests. The
structural proof compares every test body and order against the saved snapshot,
so the split does not alter assertions or fixture setup. Each suite retains its
own local-storage reset and imports the shared API/render fixtures.

Focused suites pass 8/8, the full desktop suite passes 328/328 across 72 files,
and typecheck, Web build, checker 19/19, ratchet, development-standard contract
and scoped diff checks pass. Strict remains red only for the existing 12
browser-profile payload violations. Evidence: `target/effective-line-evidence/
20260924-codex-library-split/` and
`docs/status/2026-09-24-codex-library-test-split.md`. No production, Rust,
runtime-profile or release ownership moved.
