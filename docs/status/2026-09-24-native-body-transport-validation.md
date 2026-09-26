# Native response-body transport validation

Status: isolated transport experiments have completed. The later
[native response candidate](2026-09-24-native-response-candidate.md) passes 290
program tests and records 23 browser scenarios, while page-adoption compatibility
and production integration were open at this checkpoint. The later
[active integration receipt](2026-09-25-program-capture-integration.md) closes the
program-handle adoption gap; the [matrix receipt](2026-09-25-line-matrix-closure.md)
closes its preservation window. Full S06/S18 acceptance is not complete.

Continuation source: `01a0d18f-0bbb-7332-9cb9-9e9979170642`.
Evidence paths below are relative to
`target/effective-line-evidence/20260924-integration-closure/`.

## Response coverage

`cdp-target-coverage-2026-09-24T17-36-48-529Z/summary.json` compares a separate
page CDP session with Playwright response capture. Identical-URL concurrent POSTs
match by method, post-body hash, response status and body hash, including their
out-of-order completion. The four 302/307 redirect responses preserve method/body
semantics. The same-origin iframe also matches. The page-only CDP session misses
the cross-site iframe response and dedicated-worker response, both of which the
Playwright observer captures successfully.

`cdp-target-coverage-2026-09-24T17-41-21-661Z/summary.json` repeats the five
groups with public CDP child-target attachment. All nine responses match, including
the cross-site iframe and dedicated worker. Chromium target discovery confirms
that the fixture actually creates separate iframe and worker targets. This is an
experimental transport tree, not an applied replacement. It still needs production
lifecycle, page-adoption, target-churn and failure-path proof.

## Inspector buffer limits are insufficient

`cdp-cache-bound-2026-09-24T17-50-31-812Z/summary.json` records eight loopback
cases with a 65,536-byte CDP resource buffer. Ordinary oversized responses, cold
and warm disk-cache fetches, and the oversized Service Worker response are rejected
before that session returns a body. Two memory-cached scripts nevertheless return
their complete 131,076-byte bodies. One is gzip-compressed and has no
`Content-Length`; both have zero encoded transfer bytes on the cached load.
The server receives no request during either warm script load. A CDP buffer setting
alone therefore does not establish a native body-allocation bound.

The successful small network and Service Worker controls preserve Playwright's
decoded body hashes. All eight cases report their decompressed byte counts through
`Network.dataReceived.dataLength`, including both cache-bypass cases. Declared
length and encoded transfer size cannot substitute for this measurement.

`cdp-cache-bound-2026-09-24T17-52-27-786Z/summary.json` exercises an additional
decoded-byte check before `Network.getResponseBody`. Both small controls still
match; all six oversized cases skip the body command entirely. No oversized body
is returned to the experimental reader in these eight fixtures. This is positive
evidence for that narrow check, not proof of a complete production reader or an
exact browser/Node heap bound. Missing/zero byte events, decoding expansion,
revalidation, cancellation, request-session changes and adoption remain explicit
follow-ups before integration.

The first cache probe exited 1 after an unhandled timeout while reusing the script
in the same document. Its terminal failure is retained in
`cdp-cache-bound-2026-09-24T17-44-06-563Z/failure-receipt.json`. The fixture now
reloads before exercising the memory cache and observes concurrent promises with
`Promise.all`, so a failure reaches the owning cleanup. The first corrected run is
retained at `cdp-cache-bound-2026-09-24T17-45-37-782Z/`; the later run adds the
compressed cache case and decoded-byte measurements.

## Preservation and governance refresh

Each completed browser experiment rechecks all 2,488 matrix-protected inputs and
ends with zero owned sockets; the target-coverage fixture also reports zero
timers. Contexts and browsers are closed by the fixture. A process query after the
failed first probe found no remaining root Playwright Chrome process. No real
provider or user browser profile is involved.

The experimental TypeScript files remain under ignored evidence. The child-target
and cache probes are deliberately separate from the active capture owner, whose
4 MiB input/64 MiB cumulative and retention budgets remain as documented in
[the capture-budget checkpoint](2026-09-24-capture-budget-candidate.md).

`governance-refresh-2026-09-24T17-48-57-157Z/summary.json` refreshes S20 against
the current tree: candidate checker tests pass 31/31, active strict exits 1 and
candidate strict exits 0. All 2,485 measured rows match exactly after removing only
the candidate's runtime classification field. The fourteen authenticated assets
remain visible; the other 2,471 source files have no entry above 500 effective
lines. All 182 baseline records, their provenance fields and 2,502 protected
source/runtime input hashes remain unchanged. The policy migration still awaits
the explicit approval required by main-plan section 7.1.

The provider matrix remains a separate running gate; its progress and cleanup
requirements are tracked in [the verifier checkpoint](2026-09-24-line-filter-validation.md).
The two new read-only scouts failed with model-channel HTTP 503, so these results
do not claim independent agent review. No release acceptance is claimed.

Final continuation receipt at 18:02 UTC is
`native-transport-final-2026-09-24T18-02-42-376Z.json`: 2,488 protected inputs,
14 runtime-asset hashes and all 57 prior container identities/states/ports match.
The matrix has completed 70/116 filters with 574 passed tests and no empty filter;
its Perplexity chat feature is compiling, and no final matrix summary exists yet.
The four experimental files measure 244, 115, 168 and 88 effective lines. Their
TypeScript check passes after correcting the evidence runner's generic-arrow
syntax for `.mts`; the earlier TS7060 check failure did not affect the recorded
runtime proof. UTF-8/no-BOM, the Neuro development-standard contract, and both
repositories' working/index diff checks pass. Gateway status is 273 tracked plus
2,833 untracked entries; Neuro is 10 plus 203. Inherited edits remain intact.
