# Native response capture candidate

Status: historical isolated-candidate evidence. The active reader and adoption
boundary were subsequently integrated; see [the integration receipt](2026-09-25-program-capture-integration.md).
Full S06/S18 acceptance remains open. The provider matrix passed and released its
preservation window; see [the closure receipt](2026-09-25-line-matrix-closure.md).

Continuation: `01a0d18f-0bbb-7332-9cb9-9e9979170642`. Evidence paths are relative
to `target/effective-line-evidence/20260924-integration-closure/`. Candidate files
are under `native-capture-candidate/`; its dependency junction is read-only.

## Ownership and fixes

`scripts/gemini-canvas-program-handle-cdp-sessions.mjs` owns the public CDP target
tree, command routing, startup/attachment tasks, timeouts, exact listener removal
and detachment. Root and child setup are bounded. Ignored targets count towards
the 16 concurrent sessions and 256 cumulative sessions. Child sessions admit at
most 16 pending commands, and timeout or saturation closes the capture. One
routing listener per parent dispatches nested replies. Abort signals revoke
network listeners even when `Network.enable` has not completed; late creation
and configuration results retain their cleanup obligations.

`scripts/gemini-canvas-program-handle-response-capture.mjs` owns selected request
identity, cumulative decoded bytes, response adapters and native read admission.
Only a consumer calling `text()` can request a body, and only after completion
with proven decoded bytes. Defaults remain 4 MiB per body, eight native reads and
128 pending selected requests. Missing or unproven zero-byte metadata fails
closed; known empty responses need no body command. Redirects settle the original
waiter exactly once. Consumer rejection and capture failure stop their owner.
Stopping suppresses late text without releasing an already-started native read
before its protocol promise settles.

Request maps are scoped to sessions. Real Chromium demonstrated that OOPIF
navigation emits request/response headers on the parent, followed by body and
completion events on the child. The candidate transfers that ancestor-owned
request, including its method, existing waiter and prior byte charges. Separate
sessions with independently announced equal request IDs remain separate. Reverse
and more complex target migration still need coverage before integration.

| Candidate production owner | Initial candidate effective lines | Current effective lines |
| --- | ---: | ---: |
| CDP sessions | 143 | 202 |
| Response capture | 121 | 178 |

Seven test/fixture modules range from 43 to 122 effective lines. Seven supporting
evidence scripts range from 35 to 115. All 16 measured files are below 500; no
exception, policy or baseline change is involved. The separate final receipt
runner is 43 effective lines.

## Fresh verification

The original `native-red-2026-09-24T18-17-44-097Z/` run has 11 passes and five
failures. `native-focused-2026-09-24T18-24-07-056Z/` makes those same 16 cases
green. Additional red runs at `18-32-12-585Z` and `18-39-15-914Z` preserve four
session-lifecycle failures and four admission/identity failures respectively.
The final focused suite passes 36/36 at
`native-focused-2026-09-24T18-55-47-158Z/`.

`native-program-2026-09-24T19-02-06-796Z/summary.json` passes 290/290 program
tests with zero skips or cancellations: the existing 254 plus the 36 candidate
cases. This is not an active-worker full Node or packaging claim.

`native-browser-2026-09-24T19-02-19-142Z/summary.json` records 23 loopback
scenario results using the actual candidate modules. Twenty-one validate the
intended transport or rejection behavior; two characterize integration gaps.
The run snapshots its exact sources. Its verified cases include:

- Same-URL concurrent POSTs, 302/307 redirects, same-origin/cross-site frames,
  dedicated workers and nested workers, compared with Playwright response hashes.
- Six oversized network/cache/Service Worker cases rejected before any native
  body command, including memory-cached gzip without `Content-Length`.
- Small Latin-1 decoding and cold/warm revalidation preserving decoded hashes.
  A 40,000-byte Latin-1 source expands to 80,000 UTF-8 bytes and is rejected by
  the decoded-text limit after its one admitted read.
- Aborted fetches and stopping before completion with zero native body commands
  and no late publication; initial cross-site iframe navigation now matches.

All 18 observer cleanup receipts have zero owned listeners and zero pending
consumers. Final server cleanup has zero sockets/timers; browser/context owners
close. The earlier failed browser runs at `18-49-13-757Z`, `18-50-00-193Z`,
`18-52-17-859Z` and `18-53-11-880Z` remain intact. They preserve the startup
adoption failure and the cross-session event trace that diagnosed OOPIF migration.

`native-static-2026-09-24T19-03-59-338Z/summary.json` passes nine module syntax
checks, strict TypeScript checks for the seven evidence scripts, checker tests
19/19, candidate ratchet and the Neuro development-standard contract. The
candidate scan covers 2,387 files, all at most 500 effective lines; it is not the
active repository's complete 2,485-row inventory. UTF-8 without BOM and whitespace
checks pass. No JavaScript formatter is declared in the worker package.

Earlier static failures retain their receipts: Playwright declaration imports
were corrected, and the scanner rejects the candidate's dependency junction.
The successful scan temporarily relocates only that owned junction, with exact
target/path checks and restoration in `finally`. It does not modify dependencies
or weaken scanner policy. `README.zh-CN.md` was copied unchanged for checker
contracts. Static verification preserves 2,492 active input hashes; program and
browser verification each preserve the original 2,491 preparation hashes.

## Original integration boundary

The in-flight page-adoption fixture has one Playwright response and no candidate
response. It reports `gateway_program_handle_capture_failed` with
`Program-handle response capture has no request identity.` and issues zero body
commands. The request began before the new page's CDP session was attached, so
the candidate cannot prove complete request/body metadata. The page-root target
tree does not attach to the newly opened popup; awaiting readiness after adoption
cannot recover its earlier traffic. These are recorded unsupported cases, not
successful adoption acceptance.

Next work must establish bounded ownership before relevant adopted-page traffic
starts, retain exact shared state/read budgets and test cleanup across page
replacement. Do not infer a missing request method, correlate concurrent requests
only by URL, silently omit responses, or fall back to unbounded Playwright reads.
Nested protocol-envelope allocation also remains an explicit review boundary;
the current tests do not prove an exact browser/Node heap cap.

Only after these boundaries are resolved should the candidate be wired into
network/execution owners, the fixture adapter and nested-worker package contract,
then pass fresh active tests after the matrix releases its preservation window.
Full S18, S06, RC and packaged runtime/UI/Docker acceptance remain open. S20
activation still requires the explicit approval recorded in main-plan section 7.1.

## Preservation checkpoint

`native-candidate-final-2026-09-24T19-08-14-313Z.json` matches all 2,491 protected
inputs and all 57 prior container identities/states/ports. No root Playwright
Chrome fixture process remains. Both independent repositories pass working/index
diff checks. Gateway has 273 tracked plus 2,833 untracked status entries; Neuro
has 10 plus 205. Existing work remains unstaged and uncommitted by this batch.

At 19:08 UTC the same provider matrix has completed 100/116 filters with 987
passed test executions and zero empty filters. The WebsearchAPI feature is in
progress, and no final summary exists. Matrix completion, preservation and
cleanup must be checked before any active-source integration.
