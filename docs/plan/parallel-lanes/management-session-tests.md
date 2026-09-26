# Management session test ownership

Date: 2026-09-21. State: structural_green.

Scope: `apps/desktop/src/session/ManagementSessionProvider.test.tsx` (638
effective lines), new host-origin, credentials and secret-expiry test owners,
and `managementSessionTestFixtures.tsx`. The original filename retains bootstrap
and recovery tests. Share the existing API mock, authenticated session, deferred
promise factory, Providers and SessionHarness without changing their bodies.

Preserve all 14 test bodies/assertions and descriptions. Each suite retains the
same local/session storage beforeEach cleanup. Fake-timer tests keep their own
try/finally restore. Vitest discovery/configuration and production files remain
unchanged. All owners must be <=500 effective lines.

Evidence: `target/effective-line-evidence/20260921-management-session-tests/`.
Run paired original/all-four suites, desktop typecheck, exact source projection,
test inventory, checker tests/ratchet/strict, encoding and separate Git checks.
No desktop formatter is configured; retain existing source formatting. The full
goal and reserved S06 source/build ownership remain unchanged.

Result: 163/130/80/69 effective test owners and fixture 230; paired 14/14,
desktop typecheck, exact projection and independent review pass. See
`docs/status/2026-09-21-session-telemetry-owners.md`.
