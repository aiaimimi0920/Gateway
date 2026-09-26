# Management request and provider-account HTTP extraction

Date: 2026-09-11. Owner: parallel coordinator. State: `structurally_verified`.
Scope and boundaries: [management HTTP lane](../plan/parallel-lanes/management-http.md).

## Source result

| Entry | Before effective lines | After effective lines |
| --- | ---: | ---: |
| `src/http/routes/internal_requests.rs` | 2,034 | 89 |
| `src/http/routes/internal_provider_accounts.rs` | 2,021 | 54 |

The 27 owned source/test files are all at most 338 effective lines. The request
entry delegates to audit/artifact, runtime analysis, export storage, export
reports, rate-limit hotspots, anomaly policies, anomaly incidents, remediation
actions and remediation reports. Its nine production children range from 122
to 338 lines.

The provider-account entry delegates to account input, CRUD/inventory, source
profile rules, source-profile persistence, model pricing, tiering orchestration,
tiering row storage, model discovery, Accio catalog transport, model catalog
normalization, quota and redaction. Its 12 production children range from 78 to
290 lines. Existing public function/type paths, including crate-visible payload
normalization, remain explicit re-exports. No exception or baseline regeneration
was needed.

Per-file counts and SHA-256 hashes are in
[`scope.json`](../../target/effective-line-evidence/20260911-management-http/scope.json).
Both exact pre-edit sources remain in the same evidence directory as
`internal_requests.before.rs` and `internal_provider_accounts.before.rs`.

## Preservation proof

The controlled comparison checks 107 request items and 69 provider-account items
against those snapshots, plus four retained entry helpers. Expected code permits
only explicit imports/re-exports, necessary `pub(super)` visibility and official
Rust formatting. Complete formatted item bodies, including SQL, JSON fields,
string literals, clones and awaited operations, match. The four existing
provider-account tests are also unchanged apart from relocation and imports.

The review covered each new owner and both entries:

- Admission still precedes database, filesystem, cache and upstream work. Actor
  headers remain attribution data behind management authentication.
- Audit selectors retain distinct API-key, user-credential, access-key and source
  access-key identities. Incident/queue owner filters and explicit false/zero
  values retain their existing projections.
- Export, hotspot, incident and remediation operations keep their existing
  database arguments, snapshot boundaries, error mappings and operation order.
- Credential deletion, account cache warming, quota aggregation and model
  discovery retain their existing ordering, timeouts and fallback precedence.
- Source-profile validation, credential redaction, pricing removal/clamping,
  source metadata and SQL tiering mappings keep their original semantics.
- No task, lock, cancellation boundary, process, queue or object-store lifetime
  was introduced. Allocation patterns and algorithmic bounds were preserved.

The original secret-preview implementation still uses byte offsets for string
slicing. This structural checkpoint does not establish safety for non-ASCII
secret strings; that existing behavior was not changed during extraction.

The subsequent [HTTP secret preview hardening](2026-09-11-http-secret-previews.md)
reproduced and fixed these invalid-boundary panics. That separately verified
correctness patch intentionally supersedes the affected helper bodies captured
by this structural checkpoint; its original source snapshots remain unchanged.

## Fresh verification

| Gate | Observed result |
| --- | --- |
| Original management unit baseline | 22 passed |
| Real-router management HTTP baseline | 4 passed |
| Request extraction plus original provider implementation | 25 unit and 4 HTTP tests passed |
| Both extracted entries | 25 unit and 4 HTTP tests passed |
| `cargo check --offline --locked --all-targets` | Passed |
| Scoped official Rust formatter check | Passed |
| Effective-line checker tests | 19 passed |
| Effective-line ratchet | Passed |
| Gateway and Neuro `git diff --check` | Passed independently |

The three additional unit guards cover recursive credential masking with
transport metadata preservation, model-price updates/removal and invalid
cross-kind source-profile modes. They passed on the original provider-account
implementation before its extraction. The router contracts cover missing/public
credentials, forged actor headers, valid management credential forms, unavailable
PostgreSQL and explicit-token precedence. Positive admission checks select only
database-gated routes; this is not a live PostgreSQL or upstream-service test.

Commands and logs are in
`target/effective-line-evidence/20260911-management-http/`: `baseline-unit.log`,
`baseline-routes.log`, `requests-after-unit.log`, `requests-after-routes.log`,
`final-unit.log`, `final-routes.log`, `final-all-targets.log`, `scoped-fmt.log`,
`global-fmt.log`, `checker-tests.log`, `ratchet.log` and `strict-inventory.json`.
The two extraction scripts reproduce the controlled source comparisons. Source
files were checked for UTF-8 without BOM and trailing whitespace.

Global `cargo fmt --all -- --check` still reports only these reserved S06 files:
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`. Existing upstream warnings
remain outside this scope; no new owned-file warning was reported.

## Overall plan and release boundary

Strict scans 1,370 files: 31 hard, 58 mandatory and 40 soft. There are still 89
files above 700; structural clearance is 56/145 (38.6%). Strict remains red for
that recorded debt. This checkpoint completes the two selected entries only.

No release was built or modified. S06 retains its Rust/Gemini source and original
plan cursor. GWP-20260908-06 still needs an explicit source/docs freeze and shared
release-build transfer receipt. The latest recorded immutable release remains
`20260908-producer-mailbox-s06-123700`; it predates this source checkpoint.
The remaining plan, integrated release and runtime/UI/Docker acceptance stay open.
