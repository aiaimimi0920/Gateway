# Lane D: coordinator-owned package contract extraction

- State: released in a verified interim checkpoint; overall refactor remains open.
- Owner: this conversation's coordinator, not an implementation subagent.
- Source: `tests/python/test_gateway_package_contract.py`, 743 effective lines;
  scoped Git status was clean before this task.
- Scope: original test file, new `gateway_package_fixture.py`, new
  `test_gateway_package_layout_contract.py`, and the existing nested-worker
  package test. Do not edit production packager, Rust, desktop or dependencies.
- Plan: record original test failures; extract shared fixture methods and the
  cohesive layout/integrity test; centrally add the missing required PostgreSQL
  synthetic payload and remove the nested test's duplicate workaround. Keep
  behavior changes distinct from moves and preserve every original assertion.
- Verification: compare test discovery and moved method bodies, rerun focused
  package suites and worker tests, syntax/compile, official checker and diff
  gates. Keep all remaining/new files <=500 effective lines.
- Release: S06 transferred the shared build window at 2026-09-08 00:15 UTC;
  coordinator read the transfer and now owns shared validation/build work.
  Temporary fixture packages are not product releases.

## Coordinator implementation and validation

- Extracted seven fixture/provenance/subprocess methods into
  `gateway_package_fixture.py`; extracted the cohesive layout/hash contract into
  `test_gateway_package_layout_contract.py`. No production code changed.
- Pure-move checkpoint: Python AST comparison matched all 20 original method
  bodies; unittest discovery retained all 13 original tests exactly once.
- Separate correctness batch: centralized the three required PostgreSQL
  synthetic fixture files, added layout/support-record assertions and removed
  the nested-worker duplicate workaround. Two error assertions now ignore
  PowerShell's mid-word line wrapping, retaining their complete expected text.
- Original baseline: 13 tests, 7 failed (six missing-payload failures and one
  error-format mismatch). Intermediate result: 14 tests, one remaining wrapped
  rollback-message assertion. Final result: all 14 tests pass, including the
  nested-worker package contract. No original assertion was deleted to pass.
- Effective / physical lines: remaining contract 391/441 (was 743/814), fixture
  221/237, layout 163/178, nested-worker contract 47/53. All results <=500.
- Final Python compile, 16/16 Qwen/Suno tests, 19/19 checker tests, ratchet and
  global diff check passed. Strict remains exit 1: 1008 scanned, 46 hard,
  78 mandatory, 38 soft; 124 files still exceed 700 effective lines.
- Independent read-only review found no dropped tests, duplicate discovery,
  extraction regression or CI/release-discovery gap. CI and candidate verifier
  both discover `test_*.py`, so the moved layout test remains included.
- Logs: `target/effective-line-evidence/20260908-package-contracts/` contains
  `baseline.log`, `after.log` and `final.log`. Release evidence is separate.

## Release checkpoint

Candidate ID: `20260908-parallel-refactor-002354`.
Destination root: `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
The coordinator froze the actual dirty worktree, including required untracked
owners, then successfully built and packaged both executables. All 125 Node
worker tests passed; production dependency audits found zero vulnerabilities;
desktop typecheck, web build and both release builds passed.

The package passed runtime and UI integrity checks and all 10 official isolated
runtime checks. Gateway drained with exit 0; the temporary Redis container and
Gateway process were removed. A final census found all 47 pre-existing Docker
containers unchanged, no missing/extra package files, and no checksum failures.
The shared window has been returned to S06. See the
[completion report](../../status/2026-09-08-package-contract-split.md).
This is not overall S21/strict-closure or live-provider/UI-E2E acceptance.
