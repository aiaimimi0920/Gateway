# Console document and secret ownership acceptance - 2026-09-11

This production-source batch is structurally verified. It clears two more
oversized files and advances the original structural count to 50/145 (34.5%).
The full Gateway optimization and release plan remains incomplete: 95 files
still exceed 700 effective lines and the integrated release window is pending.

## Implemented ownership boundaries

`src/console/document.rs` decreased from 1,022 to 294 effective lines. It retains
the public document/diagnostic types, compiler-facing entry points, canonical
serialization and credential materialization. Private modules own document
identity/cross-reference diagnostics, URL rules and the original account-group
unit tests. Those tests still run as `console::document::tests`.

`src/console/secrets.rs` decreased from 1,614 to 350 effective lines. It retains
the wire types, redaction/resolution orchestration and secret-grant predicate.
Eight private owners separate admission, key classification, descriptors,
identity mapping, masks, pointers, URLs and candidate validation. Crate-level
`is_sensitive_key` and `redact_url_value` keep their existing import paths through
re-exports. Private helper visibility stays within the same document or secrets
ownership subtree; public data shapes and error codes are unchanged.

| Source file | Effective lines | Responsibility |
| --- | ---: | --- |
| `src/console/document.rs` | 294 | Canonical documents and stable validation entry points |
| `src/console/document/account_groups_tests.rs` | 254 | Existing explicit/default account-group identity tests |
| `src/console/document/validation.rs` | 355 | Identity, cross-reference and alias diagnostics |
| `src/console/document/validation/urls.rs` | 130 | Transport, environment substitution and URL safety |
| `src/console/secrets.rs` | 350 | Wire types and route-document secret orchestration |
| `src/console/secrets/admission.rs` | 112 | Operation shape and per-item/aggregate byte budgets |
| `src/console/secrets/classification.rs` | 135 | Sensitive-key taxonomy and non-secret metadata |
| `src/console/secrets/descriptors.rs` | 146 | Field removal, fingerprints and bounded previews |
| `src/console/secrets/identity.rs` | 308 | Stable provider/credential and unique array identities |
| `src/console/secrets/masks.rs` | 40 | Mask and generated-preview rejection |
| `src/console/secrets/pointers.rs` | 310 | RFC 6901 parsing, schema access and JSON mutation |
| `src/console/secrets/urls.rs` | 79 | Parsed/malformed URL redaction and rejection |
| `src/console/secrets/validation.rs` | 194 | Candidate identity and embedded-secret checks |

Every owned file is below 500 effective lines. No baseline, policy exclusion,
dependency, soft exception, route configuration or S06 source was changed.

## Preservation and scoped review

The extraction verifiers reconstruct the full retained roots and all new files
from the pre-edit snapshots. They compare complete formatted source and permit
only the explicitly reviewed import/module wiring, private visibility changes,
unit-test deindentation and responsibility comments. Function bodies, public
types, diagnostics, limits, error ordering and wire behavior remain unchanged.
The document verifier checks all four files; the secret verifier checks all nine.

All 23 existing Console integration-test files still match the source hashes
recorded before this production batch. No assertion was weakened and no test
was removed. Current callers in routing, revision, persistence, Redis, runtime
and management routes still use the original public paths. A scoped search
found no test source-layout assertion tied to the old monolithic filenames.

The coordinator inspected every moved block for trust boundaries, secret
exposure, state/lifetime ownership and complexity. No stateful owner, I/O,
logging, timer, task or explicit allocation site was added. Existing recursive
walks and post-serialization size checks are preserved; this is not a claim of
new whole-process memory bounds or completed resource hardening. Both attempted
default-role independent review agents failed to start because the service does
not support their configured model. No independent review is claimed.

## Fresh verification

The same pre-change and final unit gate passed 2/2:

```powershell
cargo test --offline --locked --lib console::document::tests -- --test-threads=1
```

The same seven integration targets passed 134/134 before and after the changes:

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
transaction runtime 12 and persistence 40. The existing persistence symlink test
returns early on Windows error 1314, so its link-rejection assertion remains
unexecuted even though Cargo reports the test as passed. An intermediate
document-only gate also passed 58/58 before the secret extraction.

Final gates:

- `cargo check --offline --locked --all-targets`: exit 0.
- Scoped `rustfmt --edition 2021 --check` for both roots and all child modules:
  exit 0.
- Both complete-source extraction verifiers: passed.
- `npm run test:effective-lines --prefix scripts`: 19 passed, zero failures.
- `npm run check:effective-lines --prefix scripts`: exit 0, zero violations.
- Strict inventory: exit 1; 1,323 scanned, 34 hard, 61 mandatory and 40 soft;
  95 files remain above 700.
- Gateway and Neuro `git diff --check`: exit 0, evaluated independently.
- All 13 owned source files: strict UTF-8, no BOM or NUL.
- Neuro development-standard contract: passed.

No new compiler warning was introduced. The inherited normal-library gate has
three production warnings; the unit-library gate has one. The full formatter
still exits 1 for only these S06-owned files:

- `src/upstream/gemini_canvas_runtime_mirror.rs`
- `src/upstream/gemini_canvas_runtime_mirror_tests.rs`

These differences remain recorded for their owner. A focused green gate is not
being presented as a full-repository or release-candidate green gate.

## Evidence and release status

All batch evidence is under
`target/effective-line-evidence/20260911-console-document-secrets/`:

- `document.before.rs`, `secrets.before.rs`: captured pre-edit source.
- `split-document.mjs`, `split-secrets.mjs`: controlled extraction and verifiers.
- `baseline-lib.log`, `baseline-contracts.log`: paired pre-change gates.
- `document-contracts.log`: intermediate document verification.
- `after-lib.log`, `after-contracts.log`, `all-targets-check.log`: final Cargo gates.
- `document-equivalence.log`, `secrets-equivalence.log`: full-source comparisons.
- `scoped-format.log`, `global-format.log`, `checker-tests.log`, `ratchet.log`,
  `strict-final.json`, `gateway-diff-check.log`, `neuro-diff-check.log`,
  `development-standard.log`: remaining gate evidence.
- `scope.json`: exact before-owner counts and the 13 final source hashes; this
  scoped record is not a whole-product build-provenance manifest.

Gateway remains an existing dirty independent repository. Changes in this batch
are confined to the two production roots, 11 new child files and Gateway progress
documents; no staging, commit, reset or sibling-project edit was performed.
The original S06 plan cursor remains owned by its executor.

The latest existing release directory was refreshed on disk and remains
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\20260908-producer-mailbox-s06-123700`,
with its manifest present. It predates this source batch and is not claimed to
contain it. No new release was built: GWP-20260908-06 still has no explicit
source/docs freeze and shared-build transfer receipt. A later integrated build
must include all new untracked owners, pass its full required gates, and create
a new immutable package with runtime evidence in the designated release root.
