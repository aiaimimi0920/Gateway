# Credential, access and internal Gateway management owners

Owner: parallel coordinator. State: `structurally_verified`. Date: 2026-09-11.

Scope is `src/http/routes/internal_provider_credentials.rs` (1,005 effective
lines), `src/http/routes/internal_access.rs` (825), `src/http/routes/internal_gateway.rs`
(945), their private directories and `tests/internal_management_access_contract.rs`.
The latest accepted request/provider-account checkpoint has 89 files above 700.
S06, router wiring, database implementations and the original plan cursor are
outside this write scope.

Credential boundaries follow listing, mutations, folder synchronization, quota,
route-config fallback, public views, display metadata and payload normalization.
Access boundaries follow catalog, bundles, access keys and route diagnostics.
Internal Gateway boundaries follow project row storage, project API access,
user credentials, credential caching and usage reporting. The central management
admission functions remain in their original entry.

Preserve public paths, DTO fields/aliases, SQL arguments, owner/project/tenant
scopes, cached quota versus refresh behavior, ordered bounded collection, folder
sync and deletion order, cache TTLs, masking and failure mappings. Add focused
metadata and real-router admission contracts before moving production items.
Copies of all three original sources are under
`target/effective-line-evidence/20260911-management-access/`.

All new owners must remain below 500 effective lines. Keep source-equivalence
proofs explicit, including any necessary rebasing of sibling module paths.
Serialize Cargo gates; run paired unit/HTTP tests, all-targets compilation,
scoped/global formatter checks, checker tests, ratchet, strict inventory,
encoding/whitespace checks and independent Gateway/Neuro Git checks. Release
acceptance still requires the separate GWP-20260908-06 freeze/transfer receipt.

The three entries now contain 34, 34 and 60 effective lines respectively. All
25 owned source/test files are at most 302. Controlled source comparison preserves
123 moved production items, five retained helpers, eight original tests and two
test helpers. The credential quota loader has one explicit sibling-path rebasing.
Paired management unit 27/27 and HTTP contract 8/8 gates, all-targets compilation,
scoped formatter, checker 19/19, ratchet and independent Git checks pass. Strict
now reports 86 files above 700. Global formatting still reports only the two
reserved S06 runtime-mirror files. No release was built.

Acceptance and immutable source evidence:
[management access checkpoint](../../status/2026-09-11-management-access.md).
