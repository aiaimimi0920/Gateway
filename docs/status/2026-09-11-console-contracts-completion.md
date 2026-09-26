# Console contract decomposition acceptance - 2026-09-11

The Console test lane is structurally verified. The full Gateway optimization
plan is still incomplete: 97 files exceed 700 effective lines, the full formatter
has two out-of-scope failures, and no new product release has been built.

This continues `01a08d36-ff48-7503-8e70-ef104fb41c81`, which had already moved
three document-test groups before stopping. This continuation finished the six
remaining groups, validated the inherited management/probe/transaction splits,
and separately decomposed the remaining persistence contract suite.

## Structure and preserved behavior

The document baseline was 2,085 effective lines. Its existing integration target
now registers nine private modules and retains the shared YAML and diagnostic
fixtures. All 58 test names, attributes and complete bodies match the captured
baseline after the same Rust formatter. The environment lock and scoped variable
restoration moved only with the URL tests and also match exactly.

The persistence baseline was 1,060 effective lines. It now has five private
owners under the same integration target. All 40 test bodies, the three Windows
test cfg attributes, temporary-directory ownership, backend spies, transaction
factories and recovery fixture blocks remain unchanged. The inherited 36
management, secret-grant, probe and transaction test bodies also match their
pre-continuation snapshots after formatting.

Imports, module registration and succinct responsibility comments are the only
changes outside those moves and formatting. The pointer suite's missing `json!`
import was caught by the first post-split compile and restored before acceptance.
Production APIs, Rust/Gemini implementations, dependencies, baseline policy and
soft-exception records were not edited.

All 23 source files in the completed lane are below 500 effective lines:

| Path under `tests/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `console_document_contract.rs` | 41 | Shared document fixtures and module registration |
| `console_document_contract/validation.rs` | 285 | Strict validation, tolerant loading and document identity |
| `console_document_contract/canonical_revision.rs` | 170 | Canonical bytes and revision identity |
| `console_document_contract/redaction.rs` | 277 | Secret-free management and Debug views |
| `console_document_contract/secret_operations.rs` | 313 | Patch shape, replacement and clear semantics |
| `console_document_contract/secret_identity.rs` | 289 | Provider and credential identity during secret reuse |
| `console_document_contract/secret_arrays.rs` | 188 | Unique structural identity for array secret reuse |
| `console_document_contract/secret_pointers.rs` | 117 | RFC 6901 escaping and array-token validation |
| `console_document_contract/resource_limits.rs` | 204 | Document, patch and traversal admission bounds |
| `console_document_contract/urls.rs` | 253 | URL safety, transport rules and scoped environment substitution |
| `console_management_contract.rs` | 404 | Authentication, revision and response contracts |
| `console_management_sensitive_contract.rs` | 186 | Exact context-bound secret grants |
| `console_credential_probe_contract.rs` | 307 | Controlled loopback probes and secret gates |
| `console_contract_support/mod.rs` | 442 | Synthetic auth, in-memory Redis and probe fixtures |
| `console_transaction_contract.rs` | 70 | Namespaced Redis key layout |
| `console_transaction_runtime_contract.rs` | 387 | Runtime YAML/Redis/journal outcomes and recovery |
| `console_transaction_support/mod.rs` | 322 | Isolated transaction harness and explicit Redis outcomes |
| `console_persistence_contract.rs` | 52 | Directory/transaction fixtures and module registration |
| `console_persistence_contract/yaml_snapshots.rs` | 214 | YAML backups, immutable revisions and replacement preconditions |
| `console_persistence_contract/journal.rs` | 256 | Framing, phases and journal admission limits |
| `console_persistence_contract/recovery.rs` | 275 | Verified rollback and Redis recovery deferral |
| `console_persistence_contract/atomic_backend.rs` | 198 | Native replacement flags and injected publication failures |
| `console_persistence_contract/platform_paths.rs` | 108 | Platform path rejection and OS writer locks |

The coordinator reviewed each owner for preserved trust boundaries, fixture-only
I/O, resource lifetimes and bounded test allocations. No new production state,
task, queue, network endpoint or blocking path was introduced. Existing synthetic
backend behavior remains a test instrument, not a new production replacement
implementation. The default review agent could not start because the service
rejected its configured model; no independent review is claimed.

## Fresh verification

The six-target group passed before and after document extraction:

```powershell
cargo test --offline --locked `
  --test console_document_contract `
  --test console_management_contract `
  --test console_management_sensitive_contract `
  --test console_credential_probe_contract `
  --test console_transaction_contract `
  --test console_transaction_runtime_contract -- --test-threads=1
```

Counts are respectively 58, 12, 2, 6, 4 and 12: 94 passed, zero failures.
The separate persistence baseline and post-split gate each report 40 passed:

```powershell
cargo test --offline --locked --test console_persistence_contract -- --test-threads=1 --nocapture
cargo check --offline --locked --all-targets
```

The final all-targets check exits 0. Cargo reports 134 passed in total, with no
test-source warnings. Three inherited production warnings remain. The
`--nocapture` persistence run exposes an important qualification: the existing
`journal_link_is_rejected_without_following_it` test returns early because file
symlink creation fails with Windows error 1314. Its symlink-rejection assertion
was not exercised; the unchanged guard must not be described as native proof.

Additional gates:

- Exact document proof: all 58 tests, fixture helpers and environment guard pass;
  the same verifier also checks all 36 inherited test bodies.
- Exact persistence proof: all 40 tests, cfg attributes and fixture blocks pass.
- Scoped `rustfmt --edition 2021 --check` for all seven integration targets and
  their child/support modules: exit 0.
- `npm run test:effective-lines --prefix scripts`: 19 passed, zero failures.
- `npm run check:effective-lines --prefix scripts`: exit 0, zero violations.
- Strict audit: exit 1, 1,312 scanned, 35 hard, 62 mandatory, 40 soft; 97 violations.
- Gateway and Neuro `git diff --check`: exit 0.
- All 23 scoped source files: strict UTF-8 decoding, no BOM, no NUL.
- Neuro `test-development-standard-contract.ps1`: passed.

The full `cargo fmt --all -- --check` exits 1 only for these S06-owned files:

- `src/upstream/gemini_canvas_runtime_mirror.rs`
- `src/upstream/gemini_canvas_runtime_mirror_tests.rs`

Their formatter differences are recorded in the shared handoff for the owning
executor. They were not changed by this test-only continuation. Source work is
still uncommitted in the existing dirty Gateway worktree; no staging, commit,
reset or sibling-project edit was performed. Neuro retains its inherited dirty
root files and six dirty submodule entries; its whitespace gate is separate
from Gateway's gate.

## Evidence and remaining work

Document evidence is under
`target/effective-line-evidence/20260911-console-completion/`:
`console_document_contract.before.rs`, `split-console-document.mjs`,
`resume-baseline-cargo.log`, `after-cargo-final.log`, `after-check.log` and
`equivalence-final.log`. The failed first compile is retained in `after-cargo.log`.
The inherited test snapshots remain under `20260911-resume-console/`.

Persistence and final gate evidence is under
`target/effective-line-evidence/20260911-console-persistence/`:
`console_persistence_contract.before.rs`, `split-persistence.mjs`,
`baseline-cargo.log`, `after-cargo.log`, `all-targets-check.log`,
`equivalence-final.log`, `scoped-format.log`, `global-format.log`,
`checker-tests.log`, `ratchet.log`, `strict-final.json`,
`scope.json`, `gateway-diff-check.log`, `neuro-diff-check.log` and `development-standard.log`.
These ignored snapshots and scripts support reproducible extraction checks;
they are not shipped as production code or added to the adoption baseline.

This continuation clears two remaining oversized test files: the outstanding
count falls from 99 to 97, and original structural clearance is now 48/145
(33.1%). This does not complete S10, S14, S21 or the overall optimization plan.

The latest existing release directory was checked on disk and remains
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\20260908-producer-mailbox-s06-123700`,
with its manifest present. It predates these test changes and was not re-certified
as a current-source release. The shared handoff still has no explicit
GWP-20260908-06 source/docs freeze and Cargo/build transfer receipt. A new
integrated version therefore remains pending that receipt, final integrated
gates, the documented immutable packaging flow and real runtime verification.
