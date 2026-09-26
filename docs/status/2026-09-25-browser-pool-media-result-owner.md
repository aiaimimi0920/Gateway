# Browser-Pool Media Result Ownership Split

Date: 2026-09-25 UTC

## Scope

The media polling owner previously handled both snapshot polling and terminal
media result finalization. Final contract refresh, asset-byte resolution,
music-stream settlement, and response assembly now belong to
`scripts/gemini-canvas-browser-pool-media-result.mjs`. Polling still owns its
deadline and resumes from a new snapshot when music settlement is pending.

Effective lines changed from 497 to 389 in
`gemini-canvas-browser-pool-media-polling.mjs`; the extracted result owner is
164 lines. The media polling fixture now injects the result finalizer into its
VM context, and the nested-worker package manifest includes the new module.

## Behavior checks

- Finalization rebuilds and stores the latest invoke contract before reselecting
  media; refreshed media retains priority and empty refreshes retain the initial
  candidates.
- Image/audio byte extraction, gateway deferral for recognized fetch failures,
  music target detection, settlement wait, and final result fields remain in the
  same order.
- A regression test verifies that a contract emitted during finalization is
  merged into the returned contract and capture state.

## Verification

- Node.js v22.22.2 syntax checks passed for the polling owner, result owner,
  media-operation fixture, and focused media-polling tests.
- Focused media-polling tests: 31/31 passed.
- Effective-line checker tests: 31/31 passed.
- Effective-line ratchet: 2,503 scanned files, 14 immutable runtime artifacts
  classified separately, zero governed source above 700 lines.
- Nested-worker package contract: 1/1 passed.
- Neuro development-standard contract passed for all six submodules.

No release artifact or runtime deployment was changed. S06/S18, the separate
unknown-length native/CDP read boundary, and final release/runtime acceptance
remain open.
