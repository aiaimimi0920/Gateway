# Lane C: Suno browser worker extraction

- State: released in interim checkpoint `20260908-parallel-refactor-002354`.
- Owner: coordinator-dispatched default agent (`/root/suno_split_pilot`).
- Original: `scripts/suno-browser-worker.mjs`, 851 effective lines.
- Write scope: old worker, `scripts/suno-browser/`,
  `scripts/tests/suno-browser-worker.test.mjs`, and this record only.
- Target: entry and every new module <=500 effective lines, preserving stdin/
  stdout, requests, media pending/completed/error classification, browser target
  selection, lease release, timers, and cleanup ownership.
- No real profile, credential, network, provider, browser, or Cargo operations.
- Coordinator acceptance: structure and focused offline contracts accepted.
- Release acceptance: official build/package/integrity and isolated runtime
  gates passed; live Suno/browser-success semantics remain a separate gate.

## Original agent submission (historical, not acceptance evidence)

Baseline: original worker 851 effective / 934 physical (board baseline, 2026-09-08).
Implementation: extracted pure input/clip/error logic and browser/page/lease logic
into `scripts/suno-browser/`, preserving native entry protocol and lifecycle.
Tests: three focused offline tests passed; all three scoped files pass `node --check`.
Changed paths: entry, `suno-browser/pure.mjs`, `suno-browser/browser.mjs`, focused test,
and this record. No real provider/browser/network/release validation performed.
Coordinator acceptance and release remain pending; coordinator must verify invalid CLI
parity and effective-line ratchet.

## Coordinator acceptance: 2026-09-08

- Restored missing page/overlay imports and exports, moved platform executable
  constants with their owner, and removed the dead transport copy from pure.
- Effective / physical lines: entry 318/347 (was 851/934), browser 399/434,
  pure 145/163, focused tests 138/149. No new >500 file.
- Ten focused tests pass: input/clip/error normalization, cookies, platform
  executable lookup, borrowed/owned page selection, visible-challenge overlay
  guard, fake EasyBrowser acquire/release, and CLI validation.
- Reconstructed clean-HEAD CLI comparison preserves complete JSON and exit 0
  for malformed JSON, array input, absent base URL and missing prompt. All 40
  function bodies match after AST-based formatting normalization.
- Node syntax, encoding, checker tests, ratchet, diff checks and the isolated
  five-module package contract pass. No dependencies or protocol fields changed.
- Remaining: complete create/poll/success/error orchestration, real browser
  semantics, and integrated release/runtime gates. Existing process-exit and
  asynchronous-finally behavior is unchanged, not newly proven safe.

## Broad Python gate follow-up

Full Python discovery exposed4 stale Suno source-location assertions: moved
browser lifecycle/selector logic and clip completion predicates were still
searched only in the executable entry. Tests now inspect the actual browser
and pure owners; forbidden-generation-mechanism checks cover all3 source files.
An additional wiring contract requires the entry's imports and calls to the
extracted browser and clip helpers. No production Suno behavior changed.
Focused5/5, ratchet and diff pass. Evidence:
target/effective-line-evidence/standalone-tests/suno-contract-after/final/ratchet
logs. This repairs test scope, not a new live-browser acceptance claim.
