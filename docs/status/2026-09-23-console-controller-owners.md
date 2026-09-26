# Console controller owners

Date: 2026-09-23. Completed: 2026-09-24 UTC. Status: `complete`.

## Scope

The coordinator owns this continuation. Extract the pilot dialog's state,
provider-probe generation, opening, and closing into `usePilotDialogSession`.
Keep the shared credential-probe recovery owner and probe execution hook in
their current dependency/effect positions.

Move authoritative route hydration and provider storage-password draft updates
into `useConsoleRouteDraft`. Preserve the hydration callback's position between
editor/secret resets and structured-row resets. Keep the refresh and unload
effects ordered as before. Adapt the existing Gemini import test caller to the
draft owner's explicit UI callbacks.

Forward existing domain-hook results instead of repeating their bindings in
the controller. Preserve all 124 public field names and types, with no leaked
internal setters or duplicate public keys. Retire the controller exception only
after the finished file is at most 500 effective lines.

## Result

The pilot session and route-draft owners now hold the extracted state and
callbacks. The controller remains the composition root, and its obsolete
501-700-line exception is removed. Effective-line counts are:

| File in `apps/desktop/src/features/console/` | Before | After |
| --- | ---: | ---: |
| `useConsoleController.ts` | 698 | 496 |
| `useConsoleRouteDraft.ts` | 218 | 281 |
| `usePilotDialogSession.ts` | New | 56 |
| `useGeminiCredentialImport.test.ts` | 71 | 81 |
| `accountManagementViewModel.test.ts` | 374 | 381 |

The account-management test correction checks selection by stable account ID.
It removes a host-locale-dependent positional assertion while preserving the
production `localeCompare` ordering.

## Baseline and evidence

Evidence root: `target/effective-line-evidence/20260923-controller-owners/`.
The `before/` directory contains 17 current-worktree sources and plan records.
The controller begins at 698 effective lines, normalized source SHA-256
`a1f09016ba3b791c01333ca746ed1500676ff12a2fb828d64645125a642e74c9`.
Gateway begins with 3,033 Git porcelain entries; Neuro begins with 211.

The original and resumed default-role read-only scout attempts failed before
inspection with HTTP 503 (no `gpt-6-luna` channel). The coordinator performs the
source and contract review; no independent approval is inferred from those runs.

Fresh acceptance evidence is in the root's `resume-20260924-0223/` directory.
Its `before/` copies preserve the inherited post-extraction state before this
verification continuation. All five source/test files and the exceptions file
remain byte-identical to those copies and are valid UTF-8 without BOM. This
continuation changes acceptance documents and local evidence, not product code.

## Verification

The original green pre-extraction baseline passed 328/328 tests after the
account-ID assertion correction. The old post-extraction run subsequently
finished with 317 passed and 11 failed: ten timeouts and one group-membership
assertion failure. Its serialized Web build did not run. The original
`vitest-after.json`, `vitest-after.log`, and `frontend-gates.json` retain that
failure evidence; the cause of those failures was not established.

Fresh acceptance used the unchanged default Vitest configuration, with no
further assertion, timeout, worker, dependency, or product-source changes:

| Gate | Result | Evidence in `resume-20260924-0223/` |
| --- | --- | --- |
| Previously failing groups suite | 4/4 passed | `vitest-groups.json` |
| Full desktop Vitest | 71 files, 328/328 passed | `vitest-full-fresh.json` |
| Structural preservation | Passed | `structural-proof.json` |
| TypeScript typecheck | Exit 0 | `typecheck.log`, `gates.json` |
| Official `build:web` | Exit 0; fresh `apps/desktop/dist/web` | `build-web.log`, `gates.json` |
| Effective-line checker tests | 19/19 passed | `checker-tests.log` |
| Effective-line ratchet | Exit 0 | `ratchet.log`, `gates.json` |
| Strict audit | Exit 1; existing payload debt | `strict.json`, `strict.log` |

The structural verifier compares against the actual pre-extraction sources and
compiler-derived contract. All 124 public field names, types, and value origins
are preserved, with zero duplicate fields. It also preserves all 25 prior
domain-hook input mappings, expanded hook/effect ordering, seven pilot state
initializers, three pilot callbacks, the storage-password callback body, and
the twelve hydration statements. The original baseline evidence is not replaced.

Real-browser acceptance used freshly built Web assets and an isolated offline
Playwright session. Before secret confirmation, a provider probe opened the
confirmation dialog and sent no probe request. After confirmation, provider
and account probes carried the expected synthetic grant and rendered results.
A delayed provider result was discarded after closing and reopening its dialog;
the reopened action remained enabled. Enabling the 30-minute schedule committed
both schedule fields, and reopening after authoritative refresh restored them.
`browser-result.json` records these checks, zero pending provider requests, and
zero uncaught page errors. The dedicated browser session was closed afterward.

The browser fixture deliberately left four telemetry endpoints unmocked:
`pressure`, `costs`, `requests/summary`, and `provider-credential-model-states`
under `/v1/internal/gateway/`. Their twelve blocked requests are recorded.
All probe data and responses were synthetic; this proves the offline browser
flows, not live-provider or packaged-runtime acceptance.

The strict scan covers 2,467 files: four above 1,500, eight at 701-1,500, and two
at 501-700. All fourteen entries are existing Suno/Udio browser-profile payloads;
their paths, effective counts, and source hashes match `strict-before.json`.
The two soft entries each have 551 effective lines. No first-party source in
the current scan exceeds 500 effective lines. Policy, baseline, exclusions,
and runtime-profile payloads were not changed to obtain this result.

The coordinator reviewed the owner boundaries, public forwarding, hydration
order, secret draft handling, and probe generation/cleanup. No confirmed
extraction regression was found. No additional I/O, secret logging, unbounded
queue, or blocking work was introduced. The frontend has no configured official
formatter command; no formatter pass is claimed. Rust was untouched by this
batch, so no new Cargo gate was run for it.

Final source matching, UTF-8/no-BOM, and scoped Git checks pass. Both Gateway and
Neuro pass `git diff --check` and `git diff --cached --check`. Gateway retains
3,035 porcelain entries, including its two pre-existing staged deletions; Neuro
retains 211 entries. No files were staged, committed, reverted, or removed.
Source/document hashes and final checks are recorded in
`resume-20260924-0223/final-state-audit.json`.

## Remaining plan work

Runtime-profile payload governance, Redis/Docker acceptance, remaining S06
work, and complete release/runtime acceptance remain open. This checkpoint
does not transfer Rust or runtime-profile write ownership.
