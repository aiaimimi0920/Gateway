# Lane F: AI Studio probe test decomposition

Owner: coordinator personally. Date: 2026-09-08.
State: structural_green; next integrated release pending a shared window.

## Boundary

Only tests and coordination records changed. Production probe, browser runtime,
Rust/Gemini, package manifests, locks and checker adoption policy are unchanged.
The original clean tracked test file had2245 effective /2485 physical lines.
The suite now has seven responsibility-oriented files, all below500 effective.

| scripts/tests/ file | Tests | Effective | Physical |
| --- | ---: | ---: | ---: |
| probe-aistudio-live-request.test.mjs | 8 | 428 | 480 |
| probe-aistudio-ui-actions.test.mjs | 5 | 356 | 394 |
| probe-aistudio-transport.test.mjs | 6 | 239 | 279 |
| probe-aistudio-rpc-capture.test.mjs | 5 | 341 | 380 |
| probe-aistudio-rpc-identity.test.mjs | 5 | 407 | 443 |
| probe-aistudio-rpc-model.test.mjs | 2 | 165 | 180 |
| probe-aistudio-rpc-replay.test.mjs | 3 | 366 | 392 |

The retained entry owns error artifacts and diagnostic summaries; UI tests own
mock interaction behavior; transport tests own proxy settings and local sockets;
RPC files own capture attribution, identity integrity, model extraction and
replay-readiness semantics respectively. There is no shared mutable fixture or
new catch-all helper module.

## Behavior and discovery proof

- Baseline34/34 pass before editing.
- Post-split34/34 pass with normal Node file-level concurrency.
- A separate post-split run with --test-concurrency=1 also passes34/34.
- TypeScript AST statement comparison: original34, current34, unique34,
  zero missing/extra/changed test statements. Embedded subprocess source strings
  and assertions remain exactly unchanged.
- Production scriptPath and child cwd resolve identically because every file
  remains a sibling in scripts/tests. In this Neuro checkout the existing child
  cwd expression resolves to Neuro, not Gateway; this pre-existing behavior is
  preserved, not silently corrected.
- All seven MJS files pass node --check and UTF8/noBOM verification.
- Gateway and Neuro root git diff --check both pass; no sibling implementation
  was changed. The production probe has no tracked diff.
- CI, Windows build, tag release and candidate verifier all select
  scripts/tests/*.test.mjs, so all new siblings are included. The old explicit
  filename now runs only its eight diagnostic tests; use the group glob below
  when requesting the full AI Studio probe suite.
- Independent read-only default-agent review found no confirmed import,
  discovery, test-state or parallel resource collision regression.
- Checker tests pass19/19; adoption ratchet exit0. Strict exit1 remains expected:
  1022 scanned,45 hard,77 mandatory,38 soft;122 total files still above700.
  One hard-debt file was cleared without baseline/exception changes.

Commands from Gateway (PowerShell enumerates filenames explicitly):

    $files = @(Get-ChildItem scripts/tests -Filter 'probe-aistudio-*.test.mjs' -File |
      Sort-Object Name | ForEach-Object FullName)
    node --test $files
    node --test --test-concurrency=1 $files
    node --check <each affected file>
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts
    npm run strict:effective-lines --prefix scripts
    git diff --check

Evidence: target/effective-line-evidence/20260908-aistudio-tests/.
before.mjs preserves the original source; baseline.log and after.log contain
the pre/post results. Observed wall times were37.72s before and11.97s after:
file-level concurrency enables shorter elapsed test runs, but this single pair
under concurrent S06 load is not a controlled benchmark or performance guarantee.

## Safety and remaining gates

Helper tests suppress the production main and use synthetic captures/fake page
objects. Failure-path CLI cases use missing or deliberately invalid browser
executables. Test invocations force process-local object storage drivers to local;
no genuine browser session/provider call is intended or claimed. Loopback TCP
and WebSocket checks still open local sockets. This is not an OS network sandbox.

Unique temporary directories and finally cleanup remain intact. Each test has
its own child process. Existing fixed loopback probe ports and subprocess timeout
limitations are unchanged; the refactor does not certify those as hardened.
No real credentials, runtime profile, existing service or release was modified.

S06 retains its ongoing ingress/diagnostic and Cargo scope. This test-only
checkpoint does not mutate the previous20260908-udio-s06-034608 package or claim
that package contains these new test files. Next integrated build/package must
use a new immutable version after S06 explicitly freezes source and documents.
Original debt progress is23/145 (15.9%); broader optimization remains incomplete.
