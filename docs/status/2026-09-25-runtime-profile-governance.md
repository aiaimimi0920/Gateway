# Runtime-profile governance migration

Status: S20 complete, 2026-09-25 UTC. The overall refactor remains in progress.

## Authorization and scope

The user replied `继续推进` after the exact five-point S20 proposal and approval
question. This continuation authorizes the reversible policy and integrity-metadata
migration in [the proposal](../plan/runtime-profile-governance-proposal.md), satisfying
the explicit governance requirement in main-plan section 7.1.

The coordinator applied exactly nine candidate governance files: checker,
classifier, policy, baseline metadata, empty exception metadata, package test
entrypoint, existing checker fixtures and two new admission-test files. The lexer
is unchanged. Authoritative documentation now describes the classifier and report
fields. No provider, browser capture, Rust, desktop or package dependency changed.

The registry SHA-256 remains
`9d898f36126d72289b729752b404787202f93f8f77cf6af6790da4dda43d68b0`;
its provenance receipt SHA-256 remains
`7f2ba612b549dbc9b3f50581594439b474621a69164c188eeadfc1dd44b4f7cb`.
Only individually named installed bytes can be classified. Byte drift, malformed
evidence, traversal, junctions and unreviewed evidence changes fail closed. New
versions, siblings and first-party paths retain the ordinary size rules. All
fourteen runtime assets remain measured; no profile state was edited or removed.

## Verification and conservation

Evidence root:
`target/effective-line-evidence/20260924-integration-closure/s20-migration-20260925/`.

- `rtk proxy npm run test:effective-lines --prefix scripts`: **31 passed**, zero
  failed, skipped or cancelled. The original nineteen scenarios remain and twelve
  admission regressions cover the new classification boundary.
- `rtk proxy npm run check:effective-lines --prefix scripts`: exit **0**.
- `rtk proxy npm run strict:effective-lines --prefix scripts`: exit **0**.
- Strict TypeScript checking of `run-s20-migration.mts`: exit **0**, using the
  repository's installed TypeScript with NodeNext, `--strict`, `--noEmit` and the
  desktop Node type roots. No JS formatter is declared by the worker package;
  fresh Node test imports cover all changed checker modules' syntax.

Both checker modes report **2,497 measured files**, four above 1,500, eight at
701-1,500 and two at 501-700. Those fourteen rows carry the authenticated runtime
classification. All **2,483 governed source files are at most 500 effective lines**.
There are no diagnostics, violations or warnings. The inventory grew by the three
new governance source/test files; every non-governance row is otherwise identical.

`before.json`, `after.json` and `conservation.json` prove that all **2,494 protected
input hashes** match, including the fourteen payloads and both provenance files.
All **182 ordered baseline records**, their original source commit/tree/kind and
the file-list digest are preserved:
`599ddb5b16e02bc479a77c7f46c3594f529c1cee7d6712bce5587076d9c0a8ff`.
Thresholds, source extensions and all exclusion lists are unchanged. The exception
list remains empty. No `--write-baseline` invocation or new adoption snapshot was
used. Both repositories' HEAD and index fingerprints are unchanged.

Gateway and Neuro working-tree and index `git diff --check` all exit 0. Existing
line-ending conversion warnings are retained. At this checkpoint Gateway has
273 tracked and 2,851 untracked status entries, including the same two staged
entries; Neuro has 10 tracked, 206 untracked and none staged.

| Source file | Effective lines before | Effective lines after |
| --- | ---: | ---: |
| `scripts/effective-code-lines.mjs` | 406 | 423 |
| `scripts/effective-code-lines-runtime-artifacts.mjs` | New | 127 |
| `scripts/tests/effective-code-lines.test.mjs` | 238 | 240 |
| `scripts/tests/effective-code-lines-runtime-artifacts.fixtures.mjs` | New | 54 |
| `scripts/tests/effective-code-lines-runtime-artifacts.test.mjs` | New | 130 |

The classifier owns admission and evidence binding, with two bounded 2 MiB
document reads, finite registry/package/file counts and descriptor cleanup in
`finally`. The scanner still owns measurements; classification changes only the
evaluation and adds separate report fields. Tests use private temporary roots and
cleanup callbacks. No network service, credential or unbounded background task is
introduced. The source/test/configuration files match the reviewed candidate
byte-for-byte and are UTF-8 without BOM or trailing whitespace.

## Rollback and remaining acceptance

Original governance files, provenance files and edited documents are retained
under `before/`. A rollback must first check for later edits, restore the saved
checker/policy/baseline/exception/package/test files together, and remove only the
three newly added governance modules whose hashes still match this migration.
It must preserve unrelated work and the fourteen installed payloads. No rollback
was executed and no Git content was staged, committed or discarded.

The completed provider matrix, native program-handle capture/browser checks and
desktop evidence are reused because this migration changes none of their product
inputs. No full suite was rerun. This closes measurement governance; it grants no
complete-package redistribution rights and does not claim browser-pool/S06/S18,
RC, immutable release, packaged runtime/UI or Docker acceptance.
