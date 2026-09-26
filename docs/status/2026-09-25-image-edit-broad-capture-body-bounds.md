# Gemini image-edit broad-capture response-body admission

Status: **hardening_green for this standalone capture owner**, updated 2026-09-25
UTC. This does not close the browser-pool CDP reader, S06/S18, or release/runtime
acceptance.

## Implemented boundary

`scripts/gemini-canvas-image-edit-broad-capture.mjs` now admits a native CDP
`Network.getResponseBody` read only when the response has a valid decoded byte
count within 4 MiB or a valid in-range `Content-Length`. It checks the returned
text or Base64-decoded body again before storing a 120,000-character preview.
HEAD and 204/205/304 responses keep an empty-body event without a native body
read. The Playwright `response.text()` path requires an in-range declared length
and rejects a returned UTF-8 body over 4 MiB.

At most eight whole-body reads run concurrently. Each completed or rejected read
releases its slot, and stop clears pending CDP request state. The capture owner
grew from 317 to 401 effective lines; its capture, lifetime, execution-lifetime,
and shared fixture files currently measure 221, 102, 73, and 145 effective lines.
All remain below 500 and retain a single capture/test-fixture responsibility.

## Verification

Using Node v22.22.2, all `gemini-canvas-image-edit-broad*.test.mjs` tests pass
71/71, and `node --check` passes for the owner and matching test modules. The
effective-line checker tests pass 31/31; the ratchet scans 2,502 files and reports
zero governed source files above 700 lines. The Neuro development-standard
contract passes.

## Remaining limits

An understated in-range length is detected only after Playwright or CDP returns
the body, so the read can still allocate the full response first. Eight
simultaneous reads cap concurrency, not aggregate bytes or process heap. The
120,000-character event preview and bounded event count reduce retained data but
do not establish a hard heap cap. Other CDP consumers, including the
browser-pool capture owner, remain separate work.
