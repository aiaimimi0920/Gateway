# Program-handle native capture integration

Status: the program-handle native response reader and page-adoption boundary are
integrated and verified. This closes the isolated candidate's adoption gap. It
does not close the separate browser-pool owner, S06/S18 as a whole, S20 policy
approval, or final release/runtime acceptance.

Continuation: `01a0d5e2-bf06-7842-afb0-5da67e1bcf76`.
Evidence below is relative to
`target/effective-line-evidence/20260924-integration-closure/`.

## Ownership and behavior

`gemini-canvas-program-handle-cdp-sessions.mjs` owns the bounded CDP session tree
for one browser context. Browser-level automatic attachment pauses new pages
before relevant traffic starts. Chromium requires flattened automatic page
sessions, so a second public, non-flattened session is configured before releasing
the automatic session. Other browser contexts are detached without enabling
capture. Page/frame/worker sessions share the limits of 16 live and 256 cumulative
sessions; each routed session permits at most 16 outstanding commands.

Worker and iframe capture keeps the automatic child-session transport and exact
ancestor identity. A direct browser attachment to a worker missed its network
events in the loopback fixture; that approach was rejected. Nested body commands
now reserve a conservative escaped-envelope budget before native serialization:
32 MiB plus framing per response and 64 MiB plus framing across active reads.
Deeply nested targets can therefore reach this limit before the 4 MiB decoded-body
limit. These admission limits do not claim an exact Chromium or Node heap cap.

`gemini-canvas-program-handle-response-capture.mjs` retains the candidate's
session-scoped request identity, decoded-byte accounting, completion-before-read,
eight native reads, 128 pending selected requests and fail-closed behavior for
missing identity or unproven body size. It supplies bounded response adapters and
WebSocket metadata to the existing parsing/retention pipeline. No Playwright
`response.text()` fallback is registered in the active worker.

The network owner listens for requests on the owned context. Execution awaits
initial readiness, retains the same owner/state/budgets while adopting pages, and
awaits stop before closing the context. `adoptPage()` waits for target setup before
subsequent automation navigation. This ordering is required: Chromium can queue
`Page.navigate` ahead of debugger resumption when `newPage()` has returned but
capture setup is still pending. Popup navigation starts under the pre-established
browser owner and preserves its first request and response.

Capture stop removes only owned listeners, aborts pending configuration and
suppresses late publication. Exact listener identity, native command timeouts,
late session creation, context isolation and pending budgets have focused proof.
The package contract now enumerates both transitive native modules and imports
the packaged network owner.

## Verification and test stop condition

- `capture-focused-2026-09-25T03-09-42-770Z/`: ten affected Node test files,
  **111 passed**, zero failures, skips or cancellations. Input snapshots match
  before and after. Unchanged parser/UI/provider suites were not rerun.
- `native-adoption-2026-09-25T03-02-57-576Z/`: seven real Chromium loopback
  scenarios pass: in-flight page replacement, first popup navigation, foreign
  context isolation, workers, nested workers, cross-site frame request migration,
  and stopping before body completion. Zero owned listeners, sockets and timers
  remain. Earlier failing traces are retained.
- `program-capture-adoption-2026-09-25T03-11-55-528Z/`: five active-owner
  scenarios pass: request/response preservation during replacement, popup state
  reuse, WebSocket metadata, awaited stop without late publication, and oversized
  rejection before any body command. Two admitted body reads, zero leftover
  listeners/sockets/timers; only isolated loopback traffic and temporary browser
  profiles were used.
- `capture-static-2026-09-25T03-14-31-521Z/`: the official checker suite passes
  **19/19**. `ratchet-corrected.log` and `ratchet.json` record exit 0. The original
  absolute output-path invocation failed; correcting it to a repository-relative
  path required no policy change or checker-test rerun. The original summary and
  failure log remain intact.
- In the same static directory, `nested-worker-package.log` records
  `python -m unittest discover -s tests/python -p test_gateway_nested_worker_package_contract.py`:
  **1/1 passed**, exit 0. The PowerShell wrapper also renders Python's stderr
  progress dot as a `NativeCommandError`; the subprocess exit and unittest result
  are successful.
- Strict TypeScript checking of the four evidence drivers passes after correcting
  declaration-only observer types. Source files are UTF-8 without BOM and pass
  whitespace/size inspection. The worker package declares no JavaScript formatter.
  Node imports in the focused suite cover the changed production modules' syntax.

Final receipt: `program-capture-final-20260925.json`. All 32 focused-test inputs
and five active-browser inputs still match their passing hashes. Both Gateway and
Neuro pass working-tree and index `git diff --check`; existing line-ending
conversion warnings were left unchanged. No owned browser fixture or runner
process remains. Gateway has 273 tracked plus 2,847 untracked status entries,
including the same two staged entries; Neuro has 10 plus 205 and none staged.
This batch did not stage, commit or discard inherited work. Verification is
complete for this scope; do not repeat it without another relevant change or
unresolved failure.

## Measurements and remaining work

| Production owner | Effective lines before | Effective lines after |
| --- | ---: | ---: |
| Network capture | 248 | 264 |
| Capture budget | 81 | 81 |
| Execution | 448 | 448 |
| CDP sessions | New active module | 279 |
| Response capture | New active module | 181 |

The fresh repository scan covers **2,494 files**. Its only entries above 500
remain the same fourteen installed third-party runtime assets: four above 1,500,
eight at 701-1,500 and two at 501-700. The other **2,480 source files have no
entry above 500**. No policy, baseline or exception was changed.

The [completed provider matrix](2026-09-25-line-matrix-closure.md) is reused.
Of its 2,488 protected inputs, this batch changes only nine Node-source/test and
package-contract inputs; no Rust/provider-feature/build input changed or went
missing. There is no reason to restart its 44-line, 116-filter Cargo matrix for
this Node integration.

The S20 proposal remains [ready for explicit approval](../plan/runtime-profile-governance-proposal.md).
The continuation verified all fourteen registry hashes, unchanged exclusions and
thresholds, and preservation of all 182 candidate baseline records. Its approval
question concerns measurement classification and integrity metadata only.
The separate browser-pool capture/hardening and final RC, immutable release and
packaged runtime/UI/Docker gates retain their own acceptance boundaries.
