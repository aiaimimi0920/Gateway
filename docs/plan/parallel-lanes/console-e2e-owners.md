# Console E2E ownership split

Checkpoint: 2026-09-09

## Scope

The former 736-effective-line console.spec.ts suite was separated without changing routes, selectors, fixture payloads or test assertions. The split is test-only and stays inside apps/desktop/e2e.

## Owners

- console.e2e.mocks.ts (429 effective): MockConsoleState, route-config/session/revision fixtures, account-group and credential-pool responses, and installConsoleApiMocks.
- console.spec.ts (40 effective): administrator bootstrap and navigation-rail/settings contracts.
- console.pool.spec.ts (273 effective): provider-card flip/lifecycle, LongCat metrics/account-library layout, and tablet scrollability.

## Verification

- Playwright static discovery: 20 tests in 4 files, preserving the original 5 console contract cases plus the existing live suites.
- Desktop TypeScript typecheck: passed.
- Effective-line ratchet: passed; 36 files above 1500, 66 files in the 701-1500 tier, 40 files in the 501-700 tier.
- git diff --check: passed.
- Full browser execution is not claimed here. The current browser execution still reports provider-card and rail assertion failures; the moved assertions were not changed and need a separate UI behavior decision.

## Release boundary

This refactor does not publish a Gateway release. Native packaging remains blocked by the recorded GWP-20260908-06 source/docs freeze and Cargo transfer receipt.
