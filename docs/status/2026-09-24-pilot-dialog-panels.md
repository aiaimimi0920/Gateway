# Pilot dialog panels

Date: 2026-09-24 UTC. Status: `complete`.

## Scope

The coordinator owns `PilotActionDialog.tsx` and four new presentation owners:
`PilotAccountProbePanel.tsx`, `PilotProviderProbePanel.tsx`,
`PilotProviderSchedulePanel.tsx`, and `PilotAccountStatsPanel.tsx`.
The 483-effective-line dialog currently renders four separate action views.
Keep its public state union and props, shared title/overlay/close controls,
branch conditions, DOM structure, translations, and callback behavior intact.
Each extracted panel receives only the state and actions needed by its view.
Session state, probe execution, and draft persistence stay in their current hooks.

## Result

The four view branches now render through narrow presentation owners while the
dialog facade retains the state union, props, title resolver, overlay/header,
branch conditions, and callback wiring. Effective-line counts are:

| File | Before | After |
| --- | ---: | ---: |
| `PilotActionDialog.tsx` | 483 | 169 |
| `PilotAccountProbePanel.tsx` | new | 97 |
| `PilotProviderProbePanel.tsx` | new | 116 |
| `PilotProviderSchedulePanel.tsx` | new | 96 |
| `PilotAccountStatsPanel.tsx` | new | 171 |

All extracted owners remain below the 500-line acceptance ceiling. The
composition proof under the evidence root confirms that the public state/props
and title function are unchanged, all four branches remain present, and panel
JSX no longer remains in the facade.

Evidence root: `target/effective-line-evidence/20260924-pilot-dialog-panels/`.
Its `before/` directory captures twelve current-worktree source, test, package,
plan, and policy inputs before this batch. Gateway begins with 3,035 porcelain
entries; Neuro begins with 211. Existing changes and staged deletions are retained.

## Verification plan

Run the existing codex actions/library integration tests before and after the
extraction. Compare the original and extracted rendered output, action wiring,
and unchanged public declarations across all four view kinds and representative
loading, error, empty, locked, and draft states. Complete typecheck, full desktop
Vitest, Web build, offline browser checks, checker tests, ratchet/strict accounting,
UTF-8/no-BOM, and independent repository diff checks.

The default-role scouts from the preceding continuation remain errored with
HTTP 503/no model channel. Review is coordinator-local; no independent approval
is inferred. Rust/Gemini and runtime-profile ownership are unchanged. This
presentation extraction does not close S06 or the full release/runtime plan.

## Verification

Evidence is retained under `target/effective-line-evidence/20260924-pilot-dialog-panels/`:

- `structural-proof.json` passes the public-contract and four-branch checks;
- the focused codex actions/library suites pass 13/13 across 2 files;
- the full desktop Vitest suite passes 328/328 across 71 files;
- TypeScript typecheck exits 0 and `npm run build:web` exits 0;
- a dedicated offline Chromium harness passes 1/1 for opening and closing all
  four panels plus the provider schedule draft commit (`browser-pilot-panels.json`);
- effective-line checker tests pass 19/19 and the adoption ratchet exits 0;
- strict accounting remains intentionally red for the existing 12 browser
  runtime/extension payloads above 700 and two 551-line payloads; no first-party
  source in this scan exceeds 500 effective lines;
- scoped UTF-8/no-BOM/trailing-whitespace checks and Gateway/Neuro staged and
  unstaged `git diff --check` all pass. The frontend has no configured official
  formatter command, so no formatter pass is claimed.

The existing broad `console.pool.spec.ts` smoke still has two unrelated fixture
metric failures and one passing case; it does not exercise this panel boundary
and was not changed. No Rust, release, runtime, Docker, or live-provider gate
was run for this presentation-only batch.
