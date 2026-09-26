# HTTP secret preview hardening

Owner: parallel coordinator. State: `complete`. Date: 2026-09-11.

Scope is `src/http/middleware.rs`,
`src/http/routes/internal_provider_accounts/redaction.rs` and
`src/http/routes/internal_provider_credentials/credential_payload.rs`, including
their focused inline tests. The router extraction and prior management extraction
are accepted checkpoints; this is a separate correctness change.
Pre-edit effective sizes are 209, 116 and 90 lines respectively. Exact source
copies were taken before adding the regression tests.

All three preview helpers slice strings at fixed byte offsets. Valid UTF-8 secret
material can cross those offsets and panic. Reproduce that behavior before
changing production code. Keep existing ASCII previews, trim/empty/non-string
handling and complete UTF-8 slice boundaries. For a partial character boundary,
return the fully redacted marker without exposing secret material. Use checked
string slices so the fix adds no full-string scan or intermediate allocation.

After the failing-before test, run the same regression suite, surrounding
management and middleware unit contracts, real-router admission/layer contracts,
all-targets compilation, formatter, checker tests, ratchet and independent Git
checks. Capture source hashes and compare all unrelated production items against
the three original snapshots under
`target/effective-line-evidence/20260911-http-secret-previews/`.

S06 source, original plan cursor, live services and releases remain outside this
scope. Structural clearance starts at 60/145; this fix does not remove size debt.

The original helpers produced three UTF-8 boundary panics; three preservation
tests already passed. All six tests now pass. Checked byte slices preserve valid
previews and fail closed at partial character boundaries. Only the three named
helpers changed; 14 other production items and seven existing tests are preserved.
Final sizes are 227, 160 and 125 effective lines, including the six new tests.

Management unit 31/31, middleware unit 9/9, three real-router targets 11/11,
all-targets compilation, scoped formatter, checker 19/19, ratchet, encoding and
independent Git checks pass. Strict remains at 85 files above 700. Global
formatting still reports only the two reserved S06 runtime-mirror files.
Acceptance: [HTTP secret preview hardening](../../status/2026-09-11-http-secret-previews.md).
