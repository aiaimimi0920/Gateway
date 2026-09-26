# Provider account and credential database ownership

Accepted structural checkpoint: 2026-09-12 04:07:40 UTC.
Owner: parallel coordinator. The overall refactor plan remains in progress.

The current dirty sources were captured before editing. Account and credential
entries decreased from 873/1,081 to 99/99 effective lines. Public view/input
types and private database rows stay in their original namespaces. Lookup,
mutations, payload storage/materialization and input normalization now have
private owners; credential probe/runtime/sync transitions have a state owner.
The parent db module, schema, callers and dependencies remain hash-identical.

## Accepted structure

| File | Effective lines |
| --- | ---: |
| src/db/provider_accounts.rs | 99 |
| provider_accounts/input.rs | 263 |
| provider_accounts/lookup.rs | 76 |
| provider_accounts/mutations.rs | 295 |
| provider_accounts/payload.rs | 70 |
| provider_accounts/qwen_tests.rs | 79 |
| provider_accounts/tests.rs | 25 |
| src/db/provider_credentials.rs | 99 |
| provider_credentials/input.rs | 55 |
| provider_credentials/lookup.rs | 288 |
| provider_credentials/mutations.rs | 264 |
| provider_credentials/payload.rs | 115 |
| provider_credentials/state.rs | 203 |
| provider_credentials/tests.rs | 103 |

All 14 files are at most 295 effective lines. The new owners introduce no new
production responsibility, dependencies or configuration.

## Preservation and review

Exact formatted-source comparison preserves all 53 functions, seven types,
28 public entry paths and nine original tests, including their full test module
paths. The account qwen_tests module has production code after it; the credential
scalar-overlay helper follows its tests. Both non-tail boundaries were handled
explicitly. The unchanged parent module preserves flat db exports and callers.

All 30 SQL statements remain inside their unchanged function bodies, with the
same binds, filters, sorting, timestamps and execution order. Twenty previously
private helpers gain only pub(super) visibility for verified cross-owner uses.
Row fields remain private in the parent. The immutable pre-edit snapshot has
140 inputs; all 138 neighboring inputs and the preceding accepted scope are
unchanged.

Lookup owners retain cooling-recovery writes and credential authentication
failure exclusions before selection. Mutation and payload owners retain current
object-storage/SQL ordering, inline/object choices, merge precedence and errors.
Input owners retain protocol/profile, execution, identity, archive and status
normalization. The state owner retains failure counters, cooldowns, health checks
and sync updates. Parameter binding, async database operations and resource
lifetimes are unchanged. Atomicity, query bounds and recovery semantics remain
separate audits; extraction does not establish stronger guarantees for them.

## Fresh verification

- Default-feature account baseline/final: 4/4, including both test modules.
- Default-feature credential baseline/final: 5/5, with exact executed test paths.
- Separate cargo check --offline --locked --all-targets: passed.
- Official scoped rustfmt: passed for all 14 files.
- Effective-line checker tests: 19/19; ratchet: passed.
- Source/public-path/test/neighbor proof, UTF-8 without BOM, whitespace and
  independent Gateway/Neuro git diff --check: passed.

All native gates are terminal. The nine focused tests exercise pure
normalization and payload merging. No live PostgreSQL or S3 validation was run
for this structural batch.

Strict scans 1,516 files: 31 hard, 37 mandatory and 40 soft. There are 68 files
above 700; accepted clearance is 77/145 (53.1%). Strict still exits 1. The global
formatter still reports only the two unchanged S06 runtime-mirror files; scoped
formatting passes. These results do not establish a green global release gate.

The acceptance capture records Gateway HEAD
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and 1,656 dirty entries: 168 modified,
one unstaged deletion, two staged deletions and 1,485 untracked. Neuro HEAD is
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and
182 untracked. These counts precede this report. Inherited changes and staged
deletions are preserved.

## Evidence and continuation

The immutable capture is
target/effective-line-evidence/20260912-provider-db-owners/scope.json.
Its before.json, original-source copies, layout/capture scripts, paired Cargo
logs, non-Cargo gate receipts and strict/ratchet inventories provide the evidence.

[Lane](../plan/parallel-lanes/provider-db-owners.md).
Object storage and provider runtime are the next independent ownership review.
S06 keeps its implementation and original cursor. GWP-20260912-01 and the
explicit source/docs freeze and shared release-build transfer remain pending.
No new release was built, live service changed or runtime-profile policy migrated.
