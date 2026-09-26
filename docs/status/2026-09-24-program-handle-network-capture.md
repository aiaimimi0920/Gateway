# Standalone program-handle network capture

Date: 2026-09-24 UTC. Status: structural_green; continuation verification complete.

Recovered conversation `01a0d18f-0bbb-7332-9cb9-9e9979170642` through its last
tool result. The preceding session had extracted `startNetworkCapture`, adapted
the test fixture and added the module to the package contract. It ended on
`ModuleNotFoundError: No module named 'gateway_package_fixture'` from invoking
the package test by its dotted name. Repository-supported unittest discovery
now runs that unchanged test successfully; no production import workaround was
added. The earlier anomaly-export metadata checkpoint is already present in
`docs/progress/MASTER.md`.

## Ownership and source preservation

| File | Effective lines | Responsibility |
| --- | ---: | --- |
| `scripts/probe-gemini-canvas-program-handle.mjs` | 496 -> 322 | Configuration, dependency initialization and live-probe wiring |
| `scripts/gemini-canvas-program-handle-network-capture.mjs` | 223 | Per-page request/response/WebSocket listeners, capture state and stop guard |
| `scripts/tests/gemini-canvas-program-handle.fixtures.mjs` | 58 | Imports the extracted owner while keeping the saved inline variant testable |
| `tests/python/test_gateway_nested_worker_package_contract.py` | 231 | Packaged module bytes, support records and checksum contract |

The factory receives the same 21 probe dependencies after the media, proxy and
capture-metadata owners initialize. Each page retains its own listener functions
and stopped flag. Page adoption continues to reuse the same accumulated state.
Repeated stop removes only that capture's listeners, and stopped captures cannot
publish delayed body or cookie results into state adopted by a new page.

Source projection compares the complete capture function against the saved
entry, allowing only the two-space owner indentation. After removing the new
import and factory wiring, every remaining entry byte matches the saved source
after newline normalization. Configuration, URL filters, RPC classification,
preview lengths, merge order and final live execution wiring are unchanged.

## Fresh verification

Evidence root:
`target/effective-line-evidence/20260924-program-handle-network-capture/resume-20260924/`.
The original entry snapshot remains in the parent directory. `run-1/` contains
the individual command logs/receipts, source hashes, strict report, test parity
and `verification-summary.json`.

- Same 12 program-handle suites: 240/240 before and 240/240 after, zero skips,
  with identical test names and outcomes. This includes network capture, stop,
  page adoption, execution cleanup, metadata, media and invocation contracts.
- Nested-worker package contract: 1/1. The copied module bytes, manifest support
  record and `checksums.sha256` entry are verified in a disposable package.
- `node --check`: entry, capture owner and test fixture pass.
- Effective-line checker tests: 19/19; ratchet passes.
- Strict: expected exit 1, 2,482 files scanned; 4 above 1,500, 8 at 701-1,500,
  and 2 at 501-700. All 14 oversized paths, effective counts and source hashes
  match the previous anomaly-export report and belong to browser profiles.
  There are no first-party files above 500 in this scan.
- Neuro development-standard contract passes.
- Gateway working-tree and index `git diff --check`, and Neuro working-tree
  `git diff --check`, pass. Source files are UTF-8 without BOM or trailing space.
- All 213 captured source/test/policy input hashes remain unchanged throughout
  verification. The disposable baseline copy is removed, and no new temporary
  testable modules remain.

The baseline runs from a disposable source copy under the scripts directory so
it resolves the installed dependencies. The live worktree is never swapped to
the old entry. No dependency installation or policy/baseline regeneration occurs.

Commands from Gateway root (the runner records fully expanded file arguments):

```powershell
rtk node target/effective-line-evidence/20260924-program-handle-network-capture/resume-20260924/verify-structure.mjs
$tests = Get-ChildItem scripts/tests/gemini-canvas-program-handle.*.test.mjs | Sort-Object Name | ForEach-Object FullName
rtk node --test --test-reporter=tap --test-concurrency=1 @tests
rtk python -m unittest discover -s tests/python -p test_gateway_nested_worker_package_contract.py -v
rtk npm run test:effective-lines --prefix scripts
rtk npm run check:effective-lines --prefix scripts
```

The scripts package does not configure an official formatter. Existing source
formatting is preserved and syntax/source projection are verified; no formatter
success is claimed. The two evidence programs are 80 and 129 effective lines.

## Boundary review and next work

The capture factory adds no import-time browser, filesystem, environment, log or
network activity. It captures existing references once per probe, introduces no
new per-event copying, and preserves listener identity and state ownership.
No new URL, body, cookie or credential output is introduced; verification uses
synthetic data and never opens the user's browser profile.

Existing `response.text()` consumption remains unbounded and is not natively
cancelled by stop; stop prevents subsequent publication. Request/response and
derived capture collections retain their existing limits or lack of limits.
These are explicit S11/S18 hardening follow-ups, requiring separate behavioral
characterization and budget/cleanup tests. This extraction does not close them.

Full S18, S06 Redis/route/Docker verification, exact runtime-payload provenance
and approved S20 governance, the integrated matrix, and S21 release/runtime/UI
acceptance remain open. No Rust, Docker, real-provider, persistent service or
immutable release operation ran in this continuation. The overall plan remains
in progress; compliant first-party files do not need arbitrary further splitting.
