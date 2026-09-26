# Protocol registry and routing family ownership

Owner: parallel coordinator. State: `structurally_verified`.
Started and verified: 2026-09-12.

Scope is `src/protocol/registry.rs`, `src/routing/protocol_resolution.rs`,
their private module/test directories, and `tests/protocol_family_contract.rs`.
The original entries measure 1,314 and 1,219 effective lines. Credential routing,
configuration, provider-account storage, S06 implementation and release ownership
are caller/coordination boundaries, not source-edit scope.

Keep registry constants and family inference at the public entry. Separate
family aliases/selectors, profile defaults and ordered profile inference. Keep
candidate selection and route-policy matching at the routing entry; separate
surface capabilities, model allow/block policy and request compatibility. Preserve
public paths, all constant values, priority behavior and URL inference ordering.
Move complete items without adding runtime behavior or changing heuristics.

Relocate all 22 registry tests and 32 routing tests unchanged. Split the latter
by candidate selection, text bridges and Gemini bridges with two shared fixtures.
Before production edits, run these suites and the 26-test credential-routing
caller suite; add public model-policy characterization for alias/wildcard matching,
allow/block precedence, unsupported families and stable deduplication order.

Original snapshots, paired test source and exact-item extraction proofs belong
under `target/effective-line-evidence/20260912-protocol-family/`. All completed
owners must stay below 500 effective lines. Repeat the same tests after extraction,
then run all-targets compilation, scoped official formatter, checker tests,
ratchet, strict inventory, encoding and independent Gateway/Neuro Git checks.
Default-build registry expectations remain default-feature gates.

Before production edits, registry 22/22, routing resolution 32/32, credential
routing 26/26 and the new public model-policy contracts 5/5 pass. The paired
contract source was captured after official formatting. Both original production
snapshots matched the pre-edit source bytes; no production edit preceded these gates.

Entries are now 91/80 effective lines, and all 15 owned files are at most 322.
Complete comparison preserves 34 moved and four retained functions, 48 constants,
all 25 public function paths, 54 original tests and two fixtures. Request
compatibility remains private to the module family through `pub(super)`.

The same 85 tests pass after extraction, with an unchanged paired public-contract
source and unchanged credential-routing caller. All-targets compilation, scoped
formatter, checker 19/19, ratchet, encoding and both Git checks pass. Strict has
81 files above 700, down from 83. See the
[acceptance report](../../status/2026-09-12-protocol-family.md).

S06 retains its source and original plan cursor. The GWP-20260908-06 source/docs
freeze and shared release-build transfer still require an explicit receipt. No
integrated release is authorized by the coordinator's scoped Cargo gates.
