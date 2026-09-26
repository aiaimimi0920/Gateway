# Request and provider-account management HTTP owners

## Starting checkpoint - 2026-09-11

Owner: the parallel coordinator. State: `structurally_verified`.
Scope: `src/http/routes/internal_requests.rs`, its private directory,
`src/http/routes/internal_provider_accounts.rs`, its private directory, and
`tests/internal_management_routes_contract.rs`.

The starting scanner measured the two entries at 2,034 and 2,021 effective lines.
The repository has 1,345 scanned files, 33 hard and 58 mandatory violations;
91 files remain above 700. Existing changes in both entries are preserved.
The S06 Rust/Gemini lane, router wiring, database implementations and original
plan cursor remain outside this write scope.

Requests will be separated into audit/artifact, runtime/provider analysis,
export storage/report, rate-limit hotspot, policy, incident, remediation action
and effectiveness owners. Query types and filter projection stay with their
domain. Provider accounts will separate CRUD, source profiles, model discovery,
tiering, pricing, quota and public redaction. Existing public paths remain valid.

Before extraction, record management admission and unavailable-database behavior
through the real router, query/owner filter contracts, and the existing four
provider account unit cases. Compare complete source blocks after every move;
preserve auth ordering, SQL arguments, wire types, redaction, awaited operations,
filter scopes and failure mappings. All new owners must remain below 500 lines.

Cargo gates are coordinator-owned and serialized after a live process check.
Final validation includes focused tests, all-targets compilation, scoped and
global formatter checks, checker tests, ratchet, strict inventory, UTF-8/no-BOM
checks and independent Gateway/Neuro Git checks. The outstanding S06 formatter
and GWP-20260908-06 release-transfer receipt remain separate open items.

## Accepted source checkpoint - 2026-09-11

The entries now contain 89 and 54 effective lines. The 21 production children
separate the stated owners; all 27 scoped source/test files are at most 338 lines.
The public entry paths remain explicit re-exports. Controlled comparisons cover
176 moved production items, four retained helpers and all four relocated tests,
allowing only imports, restricted visibility and official formatting changes.

The original 22 unit and four HTTP contract cases pass before extraction. Three
additional provider payload/pricing/source-profile guards pass against the
original provider-account implementation after the request extraction. The same
25 unit and four HTTP cases pass after both extractions. All-targets compilation,
scoped formatter, checker 19/19 and ratchet pass. Global formatting still reports
only the two reserved S06 runtime-mirror files.

Strict inventory is 1,370 files, 31 hard, 58 mandatory and 40 soft; 89 files remain
above 700. Structural clearance is 56/145 (38.6%). This finishes this source
boundary, with the remaining optimization plan and release acceptance still open.
See [acceptance and preservation evidence](../../status/2026-09-11-management-http.md).
