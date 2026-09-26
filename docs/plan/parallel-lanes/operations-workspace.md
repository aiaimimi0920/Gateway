# Lane T: operations workspace

Status: in_progress, behavioral baseline established before structural edits.
Coordinator owns OperationsWorkspace.tsx and its planned section/primitives
owners plus targeted tests. S06 retains Rust/Gemini and Cargo ownership.

Current entry measures 1403 effective lines in the repository inventory. It
combines public contracts, formatting, shared panel/section primitives, live
telemetry, request filtering/audit and anomaly/remediation/export panels.
The selected split keeps accordion state, incident ordering, controlled filter
merge and toolbar in the entry. Live and request sections become separate owners;
anomaly panels split into incident/policy, remediation and export groups so no
new owner exceeds 500 lines. Shared types move only with explicit consumers;
avoid runtime dependency cycles through the entry.

Read before implementation: canonical Neuro UI prompt/design/tokens and shell
screenshot; desktop README, approved web console design, actual package scripts,
entry public props, state transitions, Section and Panel behavior. This task
preserves existing JSX, classes, fields, state ownership and UI behavior.

New baseline tests exercise independently loading/failing panels, live-only
initial expansion, simultaneous sections, controlled filter merging, apply/reset,
collapse/reopen and independent editor/refresh locks. All 3 pass on the original
production entry. Evidence: target/effective-line-evidence/operations-workspace/
baseline-tests.log. No UI production extraction or visual acceptance yet; debt
counts remain unchanged. Source/typecheck and exact JSX extraction proof will
bracket subsequent changes. No Cargo/build/live browser/provider ran.

## Shared contracts and primitives checkpoint

Public state/props/default filters now live in operations-contracts.ts (73
effective lines). OperationsPrimitives.tsx owns the 15 existing formatting,
panel, dependency-row and accordion helpers (254). The entry re-exports its
existing public contracts for current consumers and imports those leaf owners;
neither owner imports the entry. No API contract or UI behavior was changed.

Exact source verification confirms the complete workspace function, all helper
bodies and all public declarations remain unchanged apart from exports/imports.
The same baseline 3/3 tests, desktop typecheck, ratchet and diff pass; all three
source files are UTF-8 without BOM. Evidence: verify-primitives.mjs,
primitives-tests.log, primitives-typecheck.log and primitives-ratchet.log.

Entry remains 1110 effective lines. This is an intermediate extraction and is
not counted as completed debt reduction; panel groups must still move to their
planned owners. Visual/product/release acceptance remains pending. All native
sessions are terminal; no Cargo/build or live browser/provider ran.

## Panel owner extraction

Entry is now 164 effective lines. Live/request/incident/remediation/export owners
are 237/317/263/173/155; primitives/contracts remain 254/73. All are below 500.
Accordion state, incident ordering, controlled filter merging and toolbar remain
entry-owned. Children are render-only components returning fragments, preserving
DOM order and Section's conditional mounting. Current public exports remain
available through the entry, with leaf-only runtime dependencies.

Exact proof compares all five JSX blocks and the expanded workspace function
against the captured pre-split source, preserving classes, fields, callbacks and
conditions. The first proof attempt used an ambiguous multiline Panel marker;
the verifier was corrected to include remediationEffectiveness. No production
fix was needed. An independent reviewer used an older .bak despite being given
the captured baseline; its claimed class migration is not evidence of this edit.
The exact proof confirms no class migration occurred.

Focused baseline tests 3/3, desktop typecheck, web build, ratchet and diff pass.
Full Vitest is not green: 219 passed / 24 failed across 38 files. All 24 failures
are in BrowserConsoleApp.test.tsx. Loading the captured original workspace via a
test-only Vite plugin reproduces 24 failures / 25 passes in that same suite;
the failures remain a broader console gate issue, not accepted as completion.
Evidence: panels-proof.log, verify-panels.mjs, panels-tests.log,
panels-typecheck.log, panels-vitest-all.log, panels-build-web.log,
baseline-vitest.config.ts and baseline-console-tests.log.

Structural clearance is 37/145 (25.5%); 108 files remain above 700. Inventory:
1207 files, 39 hard / 69 mandatory / 38 soft. All owners are UTF-8 without BOM.
Filled-data/incident-action coverage, visual validation and the failed global
console gate still need work. Web assets were rebuilt, but no Cargo/native build,
new immutable release, live browser or provider ran. All native sessions terminal.

## Filled-data parity checkpoint

Focused coverage now includes actionable incident ordering (case-insensitive
status, then newest first), frozen input-array preservation, callback IDs,
acknowledged/resolved action eligibility, per-row busy and global editor locks,
and stale rows retained alongside loading/error feedback. A second filled-data
case verifies the unavailable Redis queue sentinel, fractional cache rates and
four-decimal savings. The same 5/5 tests pass against both the extracted owners
and the captured original component loaded through the baseline Vite plugin.
Typecheck, ratchet and diff pass. Evidence: filled-tests.log,
filled-baseline-tests.log, filled-typecheck.log and filled-ratchet.log.

Independent diagnosis of the 24 global console failures identifies three main
areas to verify before changing them: removed synthetic preview-account seeds,
tests targeting manual save/validate controls after autosave introduction, and
group-editor selectors predating the separate credential-group workspace.
These are investigation pointers; do not restore fake product data or obsolete
controls just to satisfy stale tests. Current global tests remain failed until
their actual product contracts and assertions are reconciled.

No production behavior changed in this checkpoint. Browser/visual validation
remains next; no native build, release or live provider ran. All sessions terminal.

## Isolated browser visual checkpoint

The rebuilt web bundle was exercised through an owned loopback fixture, with
explicit 503 dependency responses and no external provider requests. Screenshots
in output/playwright/operations cover light and dark themes at 390 x 844 and
1280 x 900: desktop-live.png, mobile-filters.png, mobile-dark.png and
desktop-dark.png. Filters, reset, independent accordion collapse/reopen and
keyboard Enter activation were exercised. The measured mobile viewport, document
and content widths were all 390 pixels. Inspected states showed no overlapping
panel content; screenshots retain their actual scroll positions.

All 25 console errors were expected fixture HTTP 503 responses. The remaining
console entry was a password-form accessibility advisory. This verifies rendered
error/empty states, not filled-data browser behavior or live provider operation;
filled-data parity remains established by the paired 5/5 component tests.

The named operations-split browser closed and its PID was absent. The fixture's
stdin was closed, so its exact Node command line and listening PID were verified
before stopping that owned process; port 58525 then had no listener. Global
console failures and native release acceptance remain open.

## Console contract repair checkpoint

Two obsolete seeded-Codex assertions were reconciled with the current account
ledger: a provider without credential accounts renders an empty library, while
one configured credential renders exactly one actionable card without synthetic
demo identities. Production code is unchanged. Both old tests failed before the
edit; both revised contract tests pass. The full BrowserConsoleApp suite now has
27 passes and 22 failures across the same 49 tests. Remaining failures are still
open; this is not full desktop acceptance or completed large-file clearance.

Evidence: codex-empty-before.json, codex-console-after.json, codex-typecheck.log
and codex-ratchet.log under target/effective-line-evidence/operations-workspace.
Typecheck, ratchet and diff check pass; the modified test remains UTF-8 without
BOM and shrank rather than adding a new responsibility. The PowerShell npm
wrapper dropped option flags on the first attempt, so authoritative focused and
full-suite runs invoked Node 22 with vitest.mjs directly. No formatter script is
declared in the desktop package. The initial test fixture spread of unknown JSON
failed typecheck and was replaced with an explicit typed fixture before final
verification. All native sessions are terminal; no native build or release ran.
