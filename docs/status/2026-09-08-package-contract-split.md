# Coordinator-owned package contract split and interim release

Date: 2026-09-08. Status: this bounded split and interim package are verified;
the full Gateway refactor remains in progress.

## Personally implemented by the coordinator

The coordinator, rather than an implementation agent, split the oversized
package contract. Read-only agents checked boundaries and independently reviewed
test preservation and CI discovery.

| File under `tests/python/` | Effective lines before | Effective lines after |
| --- | ---: | ---: |
| `test_gateway_package_contract.py` | 743 | 391 |
| `gateway_package_fixture.py` | new | 221 |
| `test_gateway_package_layout_contract.py` | new | 163 |
| `test_gateway_nested_worker_package_contract.py` | 55 | 47 |

Seven fixture/provenance/subprocess methods moved into a dedicated test fixture
owner; the cohesive layout, manifest and checksum contract moved into its own
test module. The remaining file owns package rejection, rollback, source
fingerprint and integrity guards. No production packager or Rust code changed.

At the pure-move checkpoint, all 20 original methods matched by Python AST and
all 13 original tests were still discovered exactly once. Then a separate
correctness batch added the three required PostgreSQL synthetic fixture files,
asserted their package/support-record presence, and removed the nested test's
duplicate workaround. Two error assertions now tolerate Windows PowerShell
mid-word line wrapping while retaining the full expected error phrase.

The original baseline had 7 failures out of 13 tests: missing fixture payloads
and an error-format mismatch. After the changes and one further wrapped-message
correction, all 14 focused tests pass, including the nested-worker package test.
No test or assertion was dropped to obtain a passing result. CI and the release
candidate verifier use `test_*.py` discovery and include the moved layout test.

## New compiled version

Version: `20260908-parallel-refactor-002354`.

Release directory:
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\20260908-parallel-refactor-002354`.

| Executable | Bytes | SHA256 |
| --- | ---: | --- |
| `gateway.exe` | 56936960 | `a22378b7aad30eb7a20888a86c13a4e314c9d6fe7398200e798870d63b8cc9ee` |
| `gateway-ui.exe` | 11935744 | `3b11c0c97c5933e8098d707f2053b83c358a87c0a727353c92be7a0f6b50f7ae` |

The full package contains 791 files including the checksum list, with no missing
or unexpected files and no checksum mismatch after runtime testing. Both Qwen
and Suno extracted directories are included. No existing release was overwritten.

Frozen worktree source fingerprint:
`ba96fdb8e54d17dee0e9824fe062a77180987788a4a43f892b3f9441c603c0af`.
The recorded pre-build snapshot, build provenance and package manifest agree.
This is the actual dirty worktree, including required untracked source owners,
not merely HEAD. A prior pre-build check correctly rejected three final S06
documentation updates; the fresh snapshot was captured before compilation.
Acceptance documentation updates occur after packaging and do not rewrite the
immutable package or its provenance.

## Fresh verification

Commands ran through the configured RTK proxy. No production provider calls
were made.

| Gate | Result |
| --- | --- |
| Three focused Python package-test modules | 14 passed |
| Four modified/new Python modules, `py_compile` | passed |
| Full `scripts/tests/*.test.mjs` matrix, explicitly enumerated | 125 passed, 0 failed/skipped/cancelled |
| Effective-line checker tests | 19 passed |
| Effective-line ratchet | exit 0 |
| Effective-line strict audit | exit 1; historical debt remains |
| Scoped/global `git diff --check` | exit 0 |
| Official `tools/build-gateway-release.ps1` | exit 0 |
| Worker and desktop production dependency audits | both 0 vulnerabilities |
| Desktop typecheck and Web assets | passed |
| Headless Cargo release and desktop Tauri release | passed |
| Official package with explicit external root and `-SkipBuild` | exit 0 |
| Runtime `-IntegrityOnly` and UI artifact smoke | passed |
| Official isolated packaged runtime smoke | all 10 checks passed |

The headless build took 1278.42 seconds and desktop shell step 142.48 seconds
according to the official build log. No dependency manifest/lock was changed;
the official build reinstalled the locked dependency trees as documented.

The runtime smoke used an approved disposable Redis container and an unused
Gateway port (49720), not port4200. Health, readiness, model listing, request-ID
propagation, three invalid-request checks, unknown-route handling, metrics and
authorized drain all passed. The runtime exited0 after drain; a separate process
census found no remaining process using the packaged executable. The temporary
Redis container was absent afterward; all47 pre-existing Docker containers had
unchanged identity/state. Inherited database URL variables were removed only
from the smoke invocation's process environment to avoid using an existing DB.

Evidence:
`target/release-evidence/20260908-parallel-refactor-002354/` contains build,
package, worker-matrix, integrity and runtime logs, `runtime.json`, source
snapshots and build provenance. Baseline/intermediate/final Python logs are in
`target/effective-line-evidence/20260908-package-contracts/`.

## Coordination and limits

- The original S06 AI transferred the shared build window; the coordinator
  finished this release and explicitly returned the window in the handoff.
- Debt is now21/145 removed (14.5%), with124 files still above700 effective
  lines:46 hard and78 mandatory. There are38 soft-limit files. Snapshot scanned1008
  source files. This coordinator task removed one additional oversized file.
- Original milestones remain6/22 complete. This is an interim release, not
  full S11/S14/S21 completion, strict closure, or full-project test acceptance.
- Live-provider semantics, full successful browser-worker flows, visual UI E2E
  and the complete all-Rust/all-Python candidate gate were not run in this batch.
  The UI smoke checked executable integrity, not an interactive UI session.
- Gateway and its Neuro submodule entry remain dirty; no commit/push/reset was
  performed. Sibling projects were not modified or validated. No live deployment
  or existing container/service replacement was performed.
- Existing test-process timeout cleanup outside the changed fixture behavior
  remains a separate hardening topic; this extraction makes no broader lifecycle
  fix claim. All subprocesses/temporary resources used in the successful gates
  above completed or were verified cleaned up.
