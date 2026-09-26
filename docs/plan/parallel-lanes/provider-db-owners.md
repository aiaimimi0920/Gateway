# Provider account and credential database ownership

Owner: parallel coordinator. State: accepted.
Started: 2026-09-12.

Scope is src/db/provider_accounts.rs, src/db/provider_credentials.rs and
their new private owners. The captured dirty entries measured 873 and 1,081
effective lines. The complete sources, all three inline test modules, parent
exports, representative direct/flat callers and table definitions were read.
Existing db/mod.rs declarations, callers and schema remain outside this scope.

Keep public view/input types and private database rows in each entry. Extract
lookup, mutations, payload materialization/storage and input normalization.
Credential lifecycle/probe/runtime/sync writes have a separate state owner.
Keep the original account qwen_tests/tests and credential tests module paths.
Every owner must remain at most 500 effective lines.

Preserve complete production items, SQL text and bind order, sorting/filtering,
cooling recovery writes and authentication-failure exclusions, timestamps,
error strings, payload merge behavior, storage decisions and object-storage/SQL
operation order. Change private visibility only for actual cross-owner uses.
The two account test modules surround production code; the final credential
scalar-overlay helper follows its test module. Neither boundary can use a
single-tail test extractor.

Capture current sources and accepted neighbors before editing. Run fresh
default-feature account/credential baselines, then the same focused gates after
extraction. Require exact production/public-path/test proof, neighbor hashes,
all-targets compilation, scoped official formatting, checker tests, ratchet,
strict inventory, encoding and independent Gateway/Neuro Git diff checks.

Evidence directory: target/effective-line-evidence/20260912-provider-db-owners/.
The immutable scope was captured at 2026-09-12 04:07:40 UTC. Both entries now
measure 99 effective lines; all 14 files are at most 295. Exact comparison retains
53 functions, seven types, 28 public paths, 30 SQL statements and all nine tests
with their original module paths. Only 20 cross-owner private helpers gain
pub(super) visibility. All 138 neighboring inputs are unchanged.

Fresh default-feature paired account 4/4 and credential 5/5, all-targets,
scoped formatter, checker 19/19, ratchet, encoding, source proof and both Git
checks pass. All native gates are terminal. Strict scans 1,516 files: 31 hard,
37 mandatory and 40 soft; 68 remain above 700. Clearance is 77/145 (53.1%).
Global formatting still reports only the two unchanged S06 runtime-mirror files.
[Acceptance report](../../status/2026-09-12-provider-db-owners.md).

The nine existing tests exercise pure normalization/merge behavior. They do not
establish live PostgreSQL or object-storage transaction acceptance. Atomicity,
query bounds and cleanup hardening are separate audits, not extraction changes.

S06 retains its implementation and original cursor. GWP-20260912-01 and the
explicit source/docs freeze and shared release-build transfer remain pending.
No release, live profile, dependency, checker policy or baseline change is included.
