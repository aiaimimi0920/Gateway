# Model pool workspace owners

Date: 2026-09-24 UTC. Status: `complete`.

## Scope

The coordinator owns the current-worktree `ModelPoolWorkspace.tsx` boundary and
its paired `ModelPoolWorkspace.test.tsx` contract. Keep the public workspace
props, model-card DOM structure, translations, account-menu wiring, provider
reordering actions, attached account filtering, flip focus restoration, and
empty/loading presentation unchanged.

Keep cross-card state in the workspace facade: expanded model, provider
deselection, card/account menu lifetimes, flip state, live announcement, and
focus refs. Move only presentation responsibilities into narrow owners:
`ModelPoolCardView.tsx` renders one card's front/back faces and
`ModelPoolServingAccountsPanel.tsx` renders the expanded account library.
`ModelPoolCardActions.tsx`, `ModelPoolCardMetrics.tsx`, and
`ModelPoolChainBoard.tsx` remain their existing nested owners.

## Result

The facade now composes the card and attached-account owners while retaining
all routing and interaction state. The historical completion audit listed the
candidate at 624 effective lines; the actual current-worktree snapshot used for
this continuation measured 383 effective lines with the repository lexer after
the earlier nested owners were present. The before/after evidence therefore
uses that exact snapshot rather than the stale planning count:

| File | Before | After |
| --- | ---: | ---: |
| `ModelPoolWorkspace.tsx` | 383 | 168 |
| `ModelPoolCardView.tsx` | n/a | 211 |
| `ModelPoolServingAccountsPanel.tsx` | n/a | 106 |

Every changed or new production owner is below the 500-line acceptance ceiling;
the two new owners have one presentation responsibility each. The facade keeps
the same `ModelPoolWorkspaceProps` declaration and owns no account-card state
that was moved into the panel.

## Evidence

Evidence root: `target/effective-line-evidence/20260924-model-pool-workspace/`.
The root contains the pre-edit source/test snapshots, `line-counts.json`,
`structural-proof.json`, `verification-summary.json`, the strict audit, and the
source-level verifier used to assert the public props and owner boundaries.

## Verification

- Focused `ModelPoolWorkspace.test.tsx`: 1 file, 6/6 passed.
- Full desktop Vitest: 71 files, 328/328 passed.
- TypeScript typecheck: exit 0.
- `npm run build:web`: exit 0.
- Effective-line checker tests: 19/19 passed.
- Adoption ratchet: exit 0; the new owners are below the preferred/acceptable
  ceiling and no baseline or exception was changed.
- Strict audit: exit 1 only for the existing browser-profile/runtime payloads;
  no first-party source owner introduced by this continuation is a strict
  violation.
- New source files are UTF-8 without BOM and have no trailing whitespace.
- Scoped `git diff --check` and the independent Gateway/Neuro checks remain
  clean; no files were staged, committed, reverted, or removed.

No Rust, release, runtime, Docker, or live-provider gate ran for this
presentation-only continuation. The existing browser smoke fixture and S06
Rust/Gemini ownership remain outside this boundary. The two scouts supplied
candidate and ownership evidence; no independent code-review approval is
inferred beyond the coordinator's source and test review.
