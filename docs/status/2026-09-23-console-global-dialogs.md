# Browser console global dialog owner

Date: 2026-09-23. Status: `complete` for this bounded extraction.

## Scope and preservation boundary

The coordinator completed this bounded console extraction. Global provider
catalog, model mapping, credential, Gemini manual-add, pilot-action, and secret
confirmation composition now lives in `ConsoleGlobalDialogs.tsx`. Its host stays
after `AppShell`, outside the route-configuration gate. Dialog order,
conditional mounting, callbacks, and the single controller-hook invocation are
preserved.

The new host receives a typed projection of the existing controller. It does
not own state or effects. Shared account-card/removal wiring and header-slot
nullability remain in the workspace composer. No design, provider, API,
dependency, or runtime-profile behavior changes are included.

## Baseline and evidence

Evidence root:
`target/effective-line-evidence/20260923-next-owners/console-global-dialogs/`.
The `before/` directory holds the current worktree sources and coordination
records; Git HEAD is not the baseline for inherited changes.

- `BrowserConsoleApp.tsx`: 549 effective lines;
  normalized SHA-256 `35f77a451fef57343749d39db31fa1c3b51f6fdbb6c431bef98b85916802a04e`.
- `useConsoleController.ts`: 698 effective lines;
  normalized SHA-256 `a1f09016ba3b791c01333ca746ed1500676ff12a2fb828d64645125a642e74c9`.
- The fresh strict scan is `../strict-before.json`: 2,465 scanned files,
  4 above 1,500, 8 at 701-1,500, and 4 at 501-700. All 12 entries above 700 are
  browser-profile/runtime payloads; no first-party source exceeds 700.
- Git snapshots: Gateway 3,031 porcelain entries; Neuro 211 entries.
- Two default-role read-only scouts failed before inspection with HTTP 503
  (no `gpt-6-luna` channel). No independent review has been obtained.

## Verification

| Owner | Before | After |
| --- | ---: | ---: |
| `BrowserConsoleApp.tsx` | 549 | 430 |
| `ConsoleGlobalDialogs.tsx` | absent | 184 |
| `useConsoleController.ts` | 698 | 698, byte-identical |

The app's unnecessary exception was removed. The controller exception and all
checker, lexer, policy, and baseline fields are unchanged. The new host reads
41 projected controller fields; the workspace reads 90, with 7 shared fields.
Their union matches the original 124 bindings. Its controller import is
type-only, and the parent still invokes the controller hook exactly once.

- Paired behavior proof: the existing `BrowserConsoleApp*`,
  `ProviderCatalogDialog`, and `GeminiManualAddDialog` suites passed 60/60 tests
  across 14 files before and after. Reports: `vitest-before.json` and
  `vitest-after.json`. No repository test was weakened or replaced.
- `structural-proof.txt`: the global dialog JSX, workspace/shell statements,
  public wrapper, conditions, callback order, and controller bytes match the
  pre-edit worktree snapshot. Only the assembly boundary and bindings changed.
- `npm run typecheck` and `npm run build:web` passed. The fresh web bundle was
  used in the browser smoke; logs are `typecheck.log` and `build-web.log`.
  No frontend formatter script or configuration is declared in this repository;
  existing formatting was retained and whitespace checks passed.
- `npm run test:effective-lines --prefix scripts`: 19/19 passed.
  `npm run check:effective-lines --prefix scripts`: ratchet passed.
- `strict-after.json`: 2,466 scanned files; 4 above 1,500, 8 at 701-1,500,
  3 at 501-700; exit 1. Its 12 violations exactly match `../strict-before.json`.
  The controller is the sole first-party soft-limit exception; the other two
  soft entries belong to runtime payloads. No first-party source exceeds 700.
- Gateway and Neuro `git diff --check` passed. Gateway status grew from 3,031
  to 3,033 entries only for the new host and this checkpoint. Neuro's 211 status
  entries are unchanged. Existing worktree changes are retained.

## Browser evidence and review

An isolated headless browser loaded the fresh `dist/web` through local request
fulfillment and reused `e2e/console.e2e.mocks.ts`. Provider catalog, secret
confirmation, credential creation, model mapping, and pilot-action dialogs
opened and closed. Dismissing secret confirmation preserved the entered catalog
draft. Settings displayed its theme and locale controls with no account-header
actions; all dialogs were dismissed and no draft was committed.

The fixture omits four telemetry endpoints: `pressure`, `costs`,
`requests/summary`, and `provider-credential-model-states`. The offline guard
blocked those requests. An initial blanket zero-blocked-request assertion
therefore returned 1. `browser-evidence-audit.txt` verifies that the four
network errors are exactly those omitted endpoints, with zero page errors or
unexpected console errors. This is qualified offline UI evidence, not a live
telemetry or backend acceptance result. Both owned browser sessions were closed.
Snapshots, `offline-catalog.png`, and the original harness results are retained.

The CLI adapter handles normalized request paths because its sandbox omits the
`URL` global. Settings verification uses the current radiogroup semantics.
The first strict invocation used an absolute output path and returned 2;
rerunning with a repository-relative `--json` path produced the final report.
These harness adjustments did not change repository source or test behavior.

The host introduces no state, effects, requests, resource lifetime, DOM wrapper,
or new authorization/secret-handling path. Callback bodies and their conditional
mounts remain exact. The controller's lifecycle order is unchanged. The source,
configuration, and plan changes are UTF-8 without BOM. Independent agent review
was unavailable as recorded above; no new soft-limit approval was claimed.

## Remaining plan work

The unchanged controller keeps its exact existing soft-limit exception. The
runtime-payload provenance/policy decision, Redis/Docker acceptance, remaining
S06 work, and complete release/runtime acceptance remain open. This batch does
not establish whole-plan or release completion.
