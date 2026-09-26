# Console persistence and journal acceptance - 2026-09-11

This production-source batch is structurally verified. It clears two more
oversized files: persistence decreased from 2,071 to 316 effective lines and
journal from 913 to 305. The full optimization plan remains incomplete, with
93 files above 700 and no new integrated release for this checkpoint.

## Ownership and preserved contracts

Public data types remain in their original modules. Shared paths, injected
backends, read-only state, degraded-state mutexes and the writer guard's owned
file are unchanged. Associated methods were moved into cohesive private modules
without changing their signatures or public/crate-level visibility. Private
cross-owner helpers expose only the original owning subtree. The existing
`persistence::read_transaction_json` path is preserved by re-export.

| Source file | Effective lines | Responsibility |
| --- | ---: | --- |
| `src/console/persistence.rs` | 316 | Public contracts, shared state, initialization and writer guards |
| `src/console/persistence/paths.rs` | 243 | Platform path admission, link rejection and secure bootstrap |
| `src/console/persistence/validation.rs` | 120 | Canonical bytes, revision identifiers and durable metadata rules |
| `src/console/persistence/file_io.rs` | 134 | Regular-file/JSON reads, durable creation and staging primitives |
| `src/console/persistence/records.rs` | 325 | Transaction constructors, receipts and forward-only phases |
| `src/console/persistence/platform.rs` | 151 | Native replacement flags, buffers and backup preconditions |
| `src/console/persistence/first_save.rs` | 131 | Immutable first-save backup and presence record |
| `src/console/persistence/archives.rs` | 352 | Revision publication, replay, listing and integrity |
| `src/console/persistence/yaml.rs` | 339 | Guarded YAML replacement, rollback and recovery receipts |
| `src/console/persistence/atomic_json.rs` | 69 | Transaction JSON publication and uncertain-failure handling |
| `src/console/journal.rs` | 305 | Public journal contracts and single-writer orchestration |
| `src/console/journal/entry.rs` | 128 | Entry conversion and immutable metadata validation |
| `src/console/journal/validation.rs` | 166 | History ordering, phase progression and cross-references |
| `src/console/journal/storage.rs` | 175 | Durable append, bounded framing and incomplete-tail repair |
| `src/console/journal/recovery.rs` | 176 | Verified local rollback and Redis-recovery deferral |

All 15 files are below 500 effective lines. No policy, baseline, exception,
dependency, route configuration or S06 source was changed.

The controlled-source verifiers reconstruct the retained roots and every new
file from captured originals. Full formatted-source comparison allows only the
reviewed imports, module wiring, private visibility and purpose comments; method
bodies, type declarations, cfg branches and existing comments remain unchanged.
All 23 earlier Console integration-test files and 13 document/secret source files
still match their prior hashes.

The coordinator reviewed the moved code for guard and resource ownership,
filesystem authority checks and failure ordering. Native path buffers stay alive
through the same FFI calls; Windows replacement/move flags and non-Windows backup
staging are unchanged. The one writer guard still spans archive preparation,
transaction JSON, YAML replacement and phase persistence. Failed replacement
continues retaining evidence and entering recovery mode at the same points.
No new I/O operation, allocation site, state owner or algorithm was introduced.
This structural proof is not a new memory-bound or concurrency-hardening claim.

## Fresh verification

The same baseline and final unit gate passed 2/2:

```powershell
cargo test --offline --locked --lib console::persistence_lock_contract -- --test-threads=1
```

These cases exercise a single OS writer guard across the durable commit phases
and an injected journal-append failure after transaction JSON is already durable.

The same seven integration targets passed 134/134 before and after extraction:

```powershell
cargo test --offline --locked `
  --test console_document_contract `
  --test console_management_contract `
  --test console_management_sensitive_contract `
  --test console_credential_probe_contract `
  --test console_transaction_contract `
  --test console_transaction_runtime_contract `
  --test console_persistence_contract -- --test-threads=1 --nocapture
```

Counts are document 58, management 12, sensitive input 2, probe 6, transaction 4,
transaction runtime 12 and persistence 40. The existing journal file-symlink
case returns early because Windows denies link creation with error 1314. Its
link-rejection assertion was not executed, despite Cargo counting the case as
passed. The test and its guard were not changed.

Additional gates:

- Intermediate journal `cargo check --offline --locked --lib`: exit 0.
- Final `cargo check --offline --locked --all-targets`: exit 0.
- Scoped `rustfmt --edition 2021 --check`: exit 0 for both roots and children.
- Complete journal and persistence source-equivalence verifiers: passed.
- Effective-line checker tests: 19 passed, zero failures.
- Effective-line ratchet: exit 0, zero new violations.
- Strict inventory: exit 1; 1,336 files, 33 hard, 60 mandatory and 40 soft;
  93 files remain above 700.
- Gateway and Neuro `git diff --check`: exit 0, checked independently.
- Scoped sources: strict UTF-8 without BOM or NUL.
- Neuro development-standard contract: passed.

No new compiler warning was introduced; the normal library retains three
production warnings and the unit-library gate retains one. The full formatter
still reports only the existing S06-owned files
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`. It is not reported as green.
Independent review is not claimed because the required default-agent model was
unavailable in the preceding attempts; the coordinator performed the review.

## Evidence and remaining work

Evidence root:
`target/effective-line-evidence/20260911-console-persistence-journal/`.

- `persistence.before.rs`, `journal.before.rs`: pre-edit snapshots.
- `split-persistence.mjs`, `split-journal.mjs`: extraction and full-source proofs.
- `baseline-locks.log`, `baseline-contracts.log`: paired pre-change gates.
- `journal-check.log`: intermediate library compilation.
- `after-locks.log`, `after-contracts.log`, `all-targets-check.log`: final Cargo gates.
- `persistence-equivalence.log`, `journal-equivalence.log`: final source comparisons.
- `scoped-format.log`, `global-format.log`, `checker-tests.log`, `ratchet.log`,
  `strict-final.json`, `gateway-diff-check.log`, `neuro-diff-check.log`,
  `development-standard.log`: other gates.
- `scope.json`: before-owner counts and exact hashes for the 15 final sources;
  this is scoped evidence, not whole-product build provenance.

The two clearances advance original structural progress to 52/145 (35.9%).
S10, S14, S21 and the overall optimization plan are not marked complete.
Gateway and Neuro retain their inherited dirty worktrees. This batch made no
staging, commit, reset or sibling-project edit, and preserved the S06 cursor.

The latest existing release was refreshed on disk and remains
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\20260908-producer-mailbox-s06-123700`,
with its manifest present. It is not a build of this source checkpoint.
GWP-20260908-06 still lacks an explicit source/docs freeze and shared-build
transfer receipt, so no new package was built. A later integrated release must
capture the entire dirty/untracked source, including these 13 new children, and
pass the official build, immutable packaging and runtime gates.
