# Gemini canvas browser-pool test ownership split

Checkpoint: 2026-09-09

## Scope

The former 912-effective-line browser-pool contract suite was split by behavior boundary. The production script and its exported behavior are unchanged; only test ownership and the shared dynamic-import fixture moved.

## Owners

- gemini-canvas-browser-pool.fixtures.mjs (52 effective): suppress-main dynamic imports and temporary exportable test module generation.
- gemini-canvas-browser-pool.test.mjs (88 effective): authentication gate, account scoping and context reuse contracts.
- gemini-canvas-browser-pool.media.test.mjs (414 effective): image/video mode selection, media provider gates, prompt submission and retry detection.
- gemini-canvas-browser-pool.runtime.test.mjs (367 effective): browser download URLs, persistent-profile/storage-state resolution, share-entry traversal, assistant text validation and Canvas proxy fallback.

## Verification

- Focused Node test run: 29/29 passed across the three moved suites.
- Effective-line checker tests: 19/19 passed.
- Effective-line ratchet: passed; 36 files above 1500, 65 files in the 701-1500 tier, 40 files in the 501-700 tier.
- git diff --check: passed after removing an extra EOF blank line.

## Release boundary

This test-only split does not publish a Gateway release. Native packaging remains blocked by the recorded GWP-20260908-06 source/docs freeze and Cargo transfer receipt.
