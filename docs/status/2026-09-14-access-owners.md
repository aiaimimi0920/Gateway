# Access database ownership acceptance

Accepted on 2026-09-13 at 16:40:19 UTC (2026-09-14 local time), after recovering
conversation 01a09005-310e-72b2-9c73-a6da52daf255. This closes the unfinished
access structural checkpoint; the full Gateway plan remains open.

## Scope and preserved behavior

The entry `src/db/access.rs` decreases from 4,214 to 307 effective lines.
Twenty private source/test owners separate public models, row conversion,
catalog reads/writes, bundles and endpoint companions, key mutations/rotation,
balance runtime/adjustments, projection queries/cache, authentication, aggregate
membership, sticky affinity, route filtering, candidate preview and route context.

Fresh reconstruction against the frozen original preserves 129 production
definitions, 54 public paths, 24 public DTOs, all private fields, 58 SQL literals,
nine original tests and seven fixtures. The crate-visible
`bump_all_access_projection_versions` remains at the entry with its original
signature, transaction and monotonic SQL expression. Twenty-nine private helpers
gain only `pub(super)`; six test imports remain cfg-guarded. Five fixture helpers
gain test-family visibility. Test names, bodies, module paths and ignore
attributes are unchanged. All 535 neighboring inputs, including 21 Rust callers
and seven supporting contracts, match their saved hashes.

The initial candidate failed compilation because the private `keys` child
collided with the parent Redis `keys` import. The corrected candidate imports
`crate::redis::keys` explicitly in balance, projection and sticky owners. This
changes imports only; the failed gate, both candidates and correction patch are
retained and checked by the acceptance verifier.

## Size and boundary review

| Owner under src/db/access/ | Effective lines | Responsibility |
| --- | ---: | --- |
| views.rs | 118 | Row-to-public-view conversion |
| catalog.rs | 145 | Access catalog queries |
| catalog_write.rs | 145 | Capability and platform access writes |
| bundles.rs | 210 | Bundle mutation and default bundle |
| bundle_compat.rs | 291 | OpenAI-compatible endpoint companions |
| keys.rs | 283 | Key normalization, lookup and mutation |
| key_rotation.rs | 156 | Transactional credential rotation |
| balance.rs | 371 | Balance cache, decision and settlement |
| balance_adjustment.rs | 131 | Explicit balance adjustments |
| projection.rs | 293 | Projection assembly, cache and invalidation |
| projection_queries.rs | 184 | Projection database queries |
| auth.rs | 116 | Authentication lookup and validation |
| memberships.rs | 150 | Aggregate membership and tenant boundaries |
| sticky.rs | 97 | Sticky affinity Redis state |
| route_filter.rs | 126 | Endpoint and model eligibility |
| candidates.rs | 160 | Candidate and decision previews |
| route_context.rs | 250 | Route-context assembly |
| models.rs | 288 | Existing public data contracts |
| tests.rs | 354 | Unchanged access regressions |
| tests/integration_fixtures.rs | 170 | Isolated destructive test fixtures |

The entry and all twenty owners stay below 500; no exception is added. Exact
definition and SQL preservation protects input/authentication decisions, bound
queries, tenant filters, transaction/cache ordering, errors and serialization.
Extraction adds no task, lock, process, allocation policy, network operation or
resource lifetime. Existing unchecked balance arithmetic, read-modify-write
behavior and key-save projection invalidation observations remain separate
investigations; this checkpoint does not claim to have repaired them.

## Verification and evidence

The paired gates completed on 2026-09-13 before interruption:

- Access units: 6/6 passed, three integration tests ignored by the unit command.
- Routing caller: 9/9 passed.
- Key-format caller: 5/5 passed.
- Internal management and access reliability contracts: 12/12 passed.
- The three ignored access regressions were separately executed before/after
  against fresh loopback PostgreSQL and Redis containers: 3/3 passed in each
  phase, including cross-project exclusion, rotation and transaction rollback.
- Locked offline all-targets compilation passed at 08:53:49.701 UTC.
- Scoped formatting, checker 19/19, ratchet, source/encoding/cleanup proof and
  both repository diff checks passed. Warning sets match their baselines.

On resumption, `capture-candidate.mjs verify` freshly reconstructed the complete
candidate and checked saved/current source hashes, API/SQL/DTO/test preservation,
UTF-8 without BOM, sizes and 535 neighboring inputs. `capture-scope.mjs` then
checked the immutable snapshot chain, exact test identities/counts, gate/log
hashes, serialized timing, all four isolated fixture identities and their
container/volume cleanup receipts. Passed Cargo gates were not rerun merely
because the conversation resumed. A fresh idle guard found no Cargo/compiler/
formatter activity. No process was terminated in this resumption.

Evidence: `target/effective-line-evidence/20260913-access-owners/scope.json`.
SHA-256: `bf0ee20fe698631a88fdeb3f066aab444b7fb1dece6563215fb234bd1013d311`.
The original namespace failure and two rejected process guards remain preserved.
The cumulative accepted source union now contains 556 inputs.

Strict inventory: 1,759 scanned, 20 hard, 27 mandatory, 40 soft; 47 files remain
above 700, down from 48. Structural clearance is 98/145 (67.6%). Strict still
exits 1. Global formatting still exits 1 only for the unchanged reserved S06
runtime-mirror implementation and test files; scoped formatting passes.

At acceptance, Gateway HEAD is `4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`:
1,961 status entries (182 modified, one unstaged deletion, two staged deletions,
1,776 untracked). Neuro HEAD is `bf818f0324024634bc890585efb78cc8e603d11a`:
192 entries (ten modified, 182 untracked). These inherited states are preserved;
documentation publication adds its own status changes afterward.

## Remaining work

Continue with operator database ownership after inspecting its exact callers,
test contracts and current implementation. S06 ownership/cursor transfer under
GWP-20260912-01, runtime-profile governance, complete language/provider/release
gates, immutable packaging and runtime/UI/Docker acceptance remain open. This
checkpoint did not build a release or change any persistent deployment,
dependency, baseline, exception or checker policy.
