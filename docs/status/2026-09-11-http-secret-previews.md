# HTTP secret preview UTF-8 hardening

Date: 2026-09-11. Owner: parallel coordinator. State: `complete` for this scope.
Scope: [HTTP secret preview lane](../plan/parallel-lanes/http-secret-previews.md).

## Reproduction and fix

Three HTTP preview helpers used unchecked byte-index string slices. The new
regressions reproduced a panic at byte 8 in middleware, byte 2 in a short
provider-account secret, and byte 3 in a provider-credential secret. All inputs
were synthetic, valid UTF-8 strings. Before the fix, three preservation cases
passed and all three invalid-boundary cases failed.

The helpers now use checked slices and return `***` when either requested
boundary cuts a UTF-8 character. Existing byte budgets and valid outputs are
unchanged: middleware keeps 8/4-byte previews above 16 bytes; provider-account
views retain their short 2-byte or long 4/2-byte previews; provider credentials
retain 3/3-byte previews above 8 bytes. Existing trimming, empty-string and
non-string behavior remains distinct between those paths.

No full-string character scan, intermediate allocation, new dependency or shared
abstraction was added. Slice validation has constant cost. Length guards remain
before subtraction, so the suffix offsets cannot underflow. Partial boundaries
fail closed without echoing the secret or panicking.

## Scope and preservation

| File | Before effective lines | After effective lines |
| --- | ---: | ---: |
| `src/http/middleware.rs` | 209 | 227 |
| `src/http/routes/internal_provider_accounts/redaction.rs` | 116 | 160 |
| `src/http/routes/internal_provider_credentials/credential_payload.rs` | 90 | 125 |

Only `mask_sensitive`, `mask_secret_value` and `mask_secret_string` change in
production. Their signatures and imports remain unchanged. Controlled comparison
preserves all 14 other production items and seven existing middleware tests. The
six new tests cover split prefix/suffix boundaries, a short split prefix, aligned
UTF-8 boundaries, ASCII, whitespace, non-string values, recursive payload/header
masking and unaffected transport metadata.

Original source copies, hashes, exact per-file counts, comparison results and
independent Git states are in
[`scope.json`](../../target/effective-line-evidence/20260911-http-secret-previews/scope.json).
`verify-preview-scope.mjs` reproduces the allowed three-helper change check;
`source-baseline.log` also records unchanged production before applying the fix.
The earlier management extraction manifests and snapshots were not rewritten.
This hardening intentionally changes their recorded preview helper bodies.

## Fresh verification

| Gate | Observed result |
| --- | --- |
| New tests against original production | 3 passed, 3 boundary panics reproduced |
| Same regression/preservation tests after fix | 6 passed |
| Management unit contracts | 31 passed |
| Middleware unit contracts | 9 passed |
| Two management HTTP targets plus router-layer contracts | 11 passed |
| `cargo check --offline --locked --all-targets` | Passed |
| Scoped official Rust formatter | Passed |
| Effective-line checker tests | 19 passed |
| Effective-line ratchet | Passed |
| UTF-8 without BOM and scoped whitespace checks | Passed |
| Gateway and Neuro `git diff --check` | Passed independently |

The six focused cases are included in the surrounding unit totals; they are not
an additional six unique tests. HTTP tests use synthetic state and perform no
live provider or PostgreSQL operations.

Logs: `target/effective-line-evidence/20260911-http-secret-previews/`, including
`failing-before.log`, `final-regressions.log`, `management-unit.log`,
`middleware-unit.log`, `router-contracts.log`, `all-targets.log`, `scoped-fmt.log`,
`global-fmt.log`, `checker-tests.log`, `ratchet.log` and `strict-inventory.json`.

Global formatting still reports only the reserved S06 files
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`. No owned-file formatter or
compilation failure remains.

## Remaining plan

Strict is unchanged at 1,399 scanned files, 31 hard, 54 mandatory and 40 soft;
85 files remain above 700. Structural clearance remains 60/145 (41.4%). No
exception or baseline regeneration was required.

This closes the three HTTP preview panic paths only. S06 ownership, its two
formatting differences and the GWP-20260908-06 source/docs freeze and build-transfer
receipt remain open. No release was built or modified. Remaining structural work
and integrated runtime/UI/Docker acceptance are not complete.
