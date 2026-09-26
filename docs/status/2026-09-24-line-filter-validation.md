# Provider-line verification and capture-budget integration

Status: the verifier, manifest repairs and capture budgets are applied and the
fresh Python/Node gates pass. The corrected provider matrix is still running.
S06/S18, S20 approval, RC and release/runtime acceptance remain open.

Latest continuation: the [native transport experiments](2026-09-24-native-body-transport-validation.md)
verify child-target response coverage and expose an inspector-buffer bypass for
memory-cached scripts. A decoded-byte precheck rejects all six oversized fixtures
before the body command. The production reader still needs lifecycle/adoption
proof. The S20 candidate was refreshed against all 2,485 current measured rows;
31/31 tests and candidate strict pass, with 182 baseline records conserved.

Continuation source: `01a0d18f-0bbb-7332-9cb9-9e9979170642`. All evidence paths
below are relative to
`target/effective-line-evidence/20260924-integration-closure/`.

## Empty filters invalidate the old matrix

The retained run `line-matrix-2026-09-24T14-16-21-414Z-981500bc/` completed 38
Cargo commands before cancellation. Nine reported zero passed tests: the three
filters each for DeepSeek, Exa and Freebuff. Earlier progress counts described
successful command exit statuses; they do not prove that those filters exercised
any behavior. The old matrix is not a passed acceptance gate.

An audit of all 116 filters in 44 manifests found 66 exact test paths that moved
during decomposition and one obsolete Kiro filter. The repairs change 67 filters
in 28 manifests. Each relocated name has a unique matching function suffix in the
full Rust test inventory. Kiro now selects the existing bearer/AWS-header behavior
test while retaining its protocol filter. The three media compile-gate filters
remain unchanged: they intentionally execute tests for compiled-out sibling
providers and are absent from the default-feature inventory.

`line-filter-mapping.json` records every old/new filter. Candidate verification
checks the exact text projection, retaining all other manifest fields. The
generated provider reference report requires only two new manifest fingerprint
fields; its official generator produces an otherwise identical report and guide.
The active validator reports 44 lines, 176 declarations, 166 unique references,
and zero issues after this metadata refresh.

## Verifier behavior and regression proof

`tools/verify-gateway-line.ps1` still streams command output and preserves native
exit-code failures. Each Cargo test invocation now independently counts executed
passing tests from Rust harness summaries. Exit zero with no summary, no matches,
or ignored-only tests fails with `Cargo command executed no tests`. Empty
auxiliary harnesses remain valid when another harness in that command ran tests.

The single-line JSON result is explicitly an array, fixing Windows PowerShell's
one-item unwrapping failure under StrictMode. Existing locked Cargo arguments,
feature/filter order, serial harness execution and `-SkipCargo` behavior remain.

The new seven-case behavioral suite first failed six cases against the old
verifier; the original exit-101 case passed. After the fix, all seven pass.
Together with the existing list-only and Cargo-throttle contracts, the isolated
suite passes 14/14. Manifest and PowerShell parser checks pass. Evidence is in
`line-verifier-2026-09-24T16-03-54-231Z/`; the red log is `line-verifier-red.log`.

## Integration and resource ownership

After candidate validation, `stop-invalid-line-matrix.ps1` checked the exact
matrix command, its runner ancestry and owned process tree before terminating
only that matrix tree. Its parent runner finalized the failure record and matched
all 2,438 protected source hashes and 57 prior container identity/state/port
records. The matrix temporary directory was removed; a fresh process query found
none of the recorded matrix, Cargo, rustc or runner process IDs remaining.
The logs and compilation cache remain. The two older cleanup-rejected temporary
directories recorded in the integration checkpoint were not touched.

`candidate-integration-receipt.json` guards 1,968 distinct active inputs before
applying 37 files: verifier/test, 28 manifests and seven capture-budget files.
All 37 active files match their validated candidates after line-ending
normalization and are UTF-8 without BOM. The subsequent provider reference
metadata correction adds one generated report to this batch.

The [capture-budget checkpoint](2026-09-24-capture-budget-candidate.md) records
the exact limits and candidate proofs: 254 focused tests, 1,959 full Node passes
with one platform skip, package 1/1, checker 19/19 and five real Chromium loopback
cases. These limits now govern the active capture owner. Single-body native
allocation through Playwright remains unbounded at this API boundary; context
closure owns cancellation. No release or live service was replaced.

| Production or verification owner | Effective lines before | Effective lines after |
| --- | ---: | ---: |
| `tools/verify-gateway-line.ps1` | 190 | 198 |
| `scripts/gemini-canvas-program-handle-capture-budget.mjs` | new | 81 |
| `scripts/gemini-canvas-program-handle-network-capture.mjs` | 223 | 248 |
| `scripts/gemini-canvas-program-handle-execution.mjs` | 441 | 448 |

The five changed/new test and fixture files have 99, 58, 19, 132 and 240 effective
lines. No new soft-limit exception or baseline change is needed.

The verifier adds one bounded counter per command and processes each output line
as it arrives; it does not retain Cargo output in memory. The manifest and report
changes add no provider inputs, credentials, network calls or lifecycle owners.
The capture budget owns cumulative accounting and read admission; the capture
owner owns listener identity and stopped-state publication, while the execution
owner retains context cleanup. Error messages do not include captured payloads.
The package fixture checks the new transitive module import. No independent agent
review is claimed: the configured scouts remained unavailable with HTTP 429.

## Active-source verification

`integrated-static-2026-09-24T16-11-40-243Z/` records syntax checks, checker 19/19
and ratchet success. Its first strict command used an unsupported absolute JSON
output path and exited 2. After correcting that evidence-runner argument,
`integrated-static-2026-09-24T16-13-33-170Z/` records the real strict exit 1:
2,485 files, four hard, eight mandatory and two soft runtime assets. The Neuro
development-standard contract and both repositories' working/index diff checks
pass. The failed command record is retained.

The first active Python run, `python-node-2026-09-24T16-09-57-034Z-cf6a7c09/`,
ran 299 tests with one failure and four skips. Its sole failure was the generated
provider reference fingerprint described above. The verifier and nested-package
tests passed. The full command was repeated after updating the two fields;
the Node phase had not started when this first runner stopped.

`python-node-2026-09-24T16-20-36-661Z-27acaa20/` records a successful Python
command: 299 tests, 295 passed and four opt-in skips. Its Node phase passes 1,958,
fails one and skips one: the unchanged real-browser navigation fixture exceeded
its existing 100 ms `page.goto` timeout. The navigation suite passes 11/11 when
run separately without changing inputs, timeout or assertions; the focused
receipt is `navigation-isolated-result.json`. This does not establish the
timeout's root cause. The full Node suite was repeated with test-process
concurrency one; the original failed run remains intact. That run preserved all
2,488 protected source/build/manifest/reference inputs and 57 prior containers,
reported no remaining owned test processes and removed its temporary directory.

`node-2026-09-24T16-38-59-928Z-79e125b1/` records the successful full Node
command with `--test-concurrency=1`: 1,960 tests, 1,959 passed, zero failed and
one Windows platform skip. No timeout, input or assertion was changed. The
production dependency audit reports zero vulnerabilities. This runner preserved
the same 2,488 protected inputs and 57 prior containers, reported no remaining
owned processes and removed its temporary directory. A subsequent hash check
found no protected-input drift. Python and Node are now green in their separate
recorded runs; the earlier failures remain available for review.

The corrected matrix is
`line-matrix-2026-09-24T16-20-36-595Z-3ed33061/`. At 18:02 UTC, 70 of 116
filters had completed, with 574 passed tests and zero empty filters; the Perplexity
chat feature was compiling. The nine previously empty DeepSeek, Exa and Freebuff
filters each now execute and pass a test. Cargo remains serialized, and the
matrix owns its source-preservation window. No passing full matrix is claimed.

Both new runners protect manifests and the provider reference report/guide as
well as code/build inputs. Matrix success requires actual executed tests for
every filter. Git status remains dirty: Gateway has 3,106 entries (273 tracked,
2,833 untracked), and Neuro has 213 (10 tracked, 203 untracked). Existing staged
and unrelated changes remain; neither repository was staged or committed.
