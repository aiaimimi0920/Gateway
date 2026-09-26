# Gateway integration and runtime-provenance checkpoint

Status: the Redis route and splitter gates are verified, the full Rust, Python
and Node commands are green, and S20 has a tested governance candidate awaiting
explicit approval. The overall refactor remains in progress. No release or live
service was replaced.

Latest continuation: the provider matrix accepted empty filters after test-module
extractions. Its old run was cancelled and retained as failed evidence; 67 filters
in 28 manifests and the verifier's zero-test/JSON handling are repaired. The
capture-budget candidate is now applied. Fresh Python (295 passed, four skipped)
and Node (1,959 passed, one skipped) commands pass. The corrected matrix later
[completed successfully](2026-09-25-line-matrix-closure.md), and the program-handle
native reader/adoption boundary is now [integrated](2026-09-25-program-capture-integration.md).
The older completed commands below retain their original source/time scope.

Continuation source: conversation `01a0d18f-0bbb-7332-9cb9-9e9979170642`.
The current coordinator retains the inherited working tree and uses a serialized
validation window. The preceding network-capture extraction remains at
`structural_green`; see [its checkpoint](2026-09-24-program-handle-network-capture.md).

## Redis fixture boundary

Port 6379 belongs to the unrelated `assetlibrary-valkey-1` container. A preflight
stopped before running tests against that service. Test configuration now accepts
`GATEWAY_STAGE_ROUTE_TEST_REDIS_URL` and `GATEWAY_SMOKE_TEST_REDIS_URL`; absent
overrides retain the localhost Redis default. The route suite's shared helper and
its two child suites use the same selected endpoint. Product configuration and
all original test scenarios/assertions are unchanged.

The exact source projection and line measurements are recorded in
`target/effective-line-evidence/20260924-integration-closure/source-2026-09-24T12-33-43-220Z-66aa784c/fixture-proof.json`:

| Test file | Effective lines before | Effective lines after |
| --- | ---: | ---: |
| `src/pipeline/stage_route/tests.rs` | 156 | 160 |
| `src/pipeline/stage_route/tests/resolution.rs` | 114 | 114 |
| `src/pipeline/stage_route/tests/account_groups.rs` | 257 | 257 |
| `tests/smoke.rs` | 446 | 447 |

The five-line increase selects isolated fixtures; it adds no production behavior,
resource owner or new dependency. URLs are not printed by the fixture selector.

## Completed verification

All evidence below is under
`target/effective-line-evidence/20260924-integration-closure/`.

- `2026-09-24T11-59-05-678Z-284713bd/`: route tests 8/8, a fresh locked debug
  `gateway` build, and the real splitter E2E 1/1 passed. The unchanged scenario
  checks readiness, replacement cutover, in-flight drain, failed readiness cleanup,
  worker crash supervision and recovery. Its runtime cwd and Redis are disposable;
  the checkout's `.env` and live route/state files are not used.
- `2026-09-24T12-19-25-454Z-9145378a/`: `cargo test --locked -- --test-threads=1`
  exited 0. The 42 reported harness/doc-test groups total 3,540 passed, zero failed,
  92 ignored and zero filtered. The library group is 3,185 passed and 36 ignored;
  the previously unverified eight route cases ran. The earlier nine subprocess
  fixture failures did not recur. Ignored integration/provider cases remain
  unverified by this command.
- `source-2026-09-24T12-33-43-220Z-66aa784c/`: exact fixture projection,
  `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, the 19 checker
  tests, ratchet, Neuro development-standard contract, Gateway working/index diff
  checks and Neuro working-tree diff check passed. The active strict audit exited
  1: 2,482 measured files, four hard, eight mandatory and two soft runtime assets.
- `python-node-2026-09-24T12-46-55-898Z-3c6d2844/`: manifest validation, Python
  discovery and the complete Node worker suite passed. Python ran 292 tests:
  288 passed and four skipped. Node ran 1,946 tests: 1,945 passed and one skipped.
  The production worker dependency audit reported zero vulnerabilities.
  Python's opt-in skips cover console Redis, PostgreSQL recovery, Docker recovery
  and splitter E2E; the last was exercised separately above. Node's skip is the
  POSIX-only credential-permission scenario on Windows.
- `desktop-2026-09-24T13-05-48-587Z-362c8d83/`: all six product gates exited 0:
  desktop typecheck, 328/328 tests across 72 files, desktop build, production audit
  (zero vulnerabilities), Tauri formatter and locked Tauri compilation. The
  post-gate process query timed out; a later read-only query found zero owned
  test processes. Source and container before/after receipts match exactly.

Both runtime runs preserved 2,438 source/build-input hashes and all 57 prior
containers' identity/state/ports. Each owned Redis container was removed, its
random loopback port released, and its temporary directory removed. Docker Server
29.5.3 and the existing 4200 development stack were available, superseding the
older Docker HTTP 500 blocker. The live source-mounted stack has its development
watcher disabled; no 4226 listener was observed during preflight.

The Python/Node run also preserved all 2,438 source/build-input hashes and 57
prior container identities/states/ports, recorded zero remaining owned processes,
and removed its temporary directory. A continuation check found no drift in those
2,438 inputs before the documentation update. The isolated Python environment
uses the declared `jsonschema==4.25.1` dependency and puts its Scripts directory
on PATH for nested PowerShell calls.

## Provider-feature regression and cleanup follow-up

`line-matrix-2026-09-24T13-25-04-130Z-57ed4480/` is a retained failed run.
Accio's enabled protocol/upstream filters passed, then the AI Studio official
feature failed to compile. The disabled Accio facade includes `event_parse.rs`
through `#[path]`; its implicit child declarations resolved beside the shared
file rather than inside `event_parse/`, selecting the wrong tests and missing
`stream_events.rs`. Two explicit child paths repair that boundary. All parsing
logic and test bodies remain exact; effective lines change from 261 to 263.
The [module-path checkpoint](2026-09-24-accio-module-paths.md) records successful
enabled Accio 19/19, disabled Accio 8/8, AI Studio 1/1, formatter, checker 19/19,
ratchet and diff checks in `accio-module-2026-09-24T13-59-25-189Z/`. The complete
provider matrix has restarted. The earlier full Rust result predates this
two-line module-path fix.

The desktop and failed matrix wrappers both timed out during their final
PowerShell process queries. Manual process inspection found no owned test
processes, but automatic approval review rejected both recursive cleanup and
a narrower exact-cache cleanup with only `blocked by policy`. The retained
desktop directory is
`C:\Users\vmjcv\AppData\Local\Temp\gateway-product-gates-E6BguV` (573 Node cache
files, 3,088,452 bytes); the retained matrix directory is
`C:\Users\vmjcv\AppData\Local\Temp\gateway-product-gates-hgaN9w` (empty).
No cleanup success is claimed for these two runs; both original failure summaries
are preserved. This does not change their recorded individual test exit codes.

## S20 provenance and unapplied candidate

`signatures-2026-09-24T12-20-12-847Z/` authenticates all fourteen current oversized
assets from six installed browser package copies. It verifies the Webstore
RSA/SHA-256 signatures and per-file tree hashes against a pinned Chromium trust
anchor, including signature and byte-mutation negative controls. This proves the
listed bytes and signed item ID/version; it does not establish complete-bundle
redistribution rights or download history.

The [governance proposal](../plan/runtime-profile-governance-proposal.md) now links
the exact versioned path/hash registry and durable provenance receipt. Its
unapplied candidate passes 31/31 checker tests and a strict preview with all
fourteen measured rows preserved. All 182 baseline records, the original file-list
digest, active governance inputs and runtime payloads are unchanged. The policy
migration still requires the explicit approval specified by main-plan section 7.1.
The candidate's `npm run test:effective-lines` entrypoint was also exercised in
the continuation: 31 passed, zero failed or skipped.

## Remaining work

The [capture-budget candidate](2026-09-24-capture-budget-candidate.md) passes
254/254 focused tests and the full Node suite (1,959 passed, zero failed, one
platform skip). Its package contract now verifies the budget module and imports
the packaged capture owner; it passes 1/1. Checker tests pass 19/19, all seven
candidate files are at most 448 effective lines, and that isolated acceptance
run preserved all 491 protected active inputs. Five real Chromium loopback cases verify normal data,
oversize rejection, concurrent read admission, listener cleanup and native read
settlement after context closure. The candidate has now been applied following
controlled cancellation of the invalid matrix; current gate results are recorded
in the verifier checkpoint above. A separate CDP experiment confirms that an
additional session's buffer limits do not constrain existing Playwright body
reads; native single-body allocation remains open.

S20 activation and post-migration proof, remaining capture hardening including
native single-body allocation, S06/S18 acceptance, provider line-matrix/RC gates,
final immutable release, and packaged runtime/UI/Docker
acceptance remain open. Existing release checkpoints retain their original scope.

The initial two read-only scouts could not run because their configured
`gpt-6-luna` channel returned HTTP 503. Two later bounded scouts returned HTTP
429; the coordinator performed the checks directly. This checkpoint claims no
independent agent review.
