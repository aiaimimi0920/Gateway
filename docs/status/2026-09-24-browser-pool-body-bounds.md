# Browser-pool response body bounds

Status: **hardening_green for the bounded body boundary**, updated 2026-09-25 UTC.
The broader S18, provider, packaged-runtime, UI, Docker and release gates remain
open.

## Implemented boundary

`scripts/gemini-canvas-browser-pool-body.mjs` is the single owner for browser-pool
body limits and Playwright/Node response adapters. Text responses are limited to
4 MiB and binary responses to 16 MiB. Limits are fixed by the worker and cannot be
expanded by a request payload.

The network capture owner checks a declared `content-length` before calling
Playwright's whole-body reader, then verifies the returned UTF-8 or binary size.
Oversized image and audio reads fail closed to the existing URL-only media record;
video responses still do not read a body. Oversized program text remains a bounded
diagnostic and cannot publish handle pairs from a rejected body.

Playwright reads reject an oversized declared length before calling `text()` or
`body()`. Missing or malformed lengths fail with 502 before a whole-body read;
declared zero does not suppress the read, and returned bytes are always checked.
HEAD and 204/205/304 responses return empty bodies before interpreting a
representation length.

Page-context fetches count actual bytes from a `ReadableStream` and cancel as soon
as the text or binary budget is crossed. Without a stream reader, missing or
malformed lengths fail before `arrayBuffer()`/`text()`; a declared in-range length
allows the whole-body fallback, whose actual result is checked after it settles.
The same boundary is used by no-key fetch, preview probes, fixture fetches, asset
extraction and navigation downloads. The existing page/capture finally blocks and
URL-only media fallback remain unchanged.

## Follow-on owner extraction

The bounded page-context fetch callback was extracted from
`scripts/gemini-canvas-browser-pool-fetch-execution.mjs` into
`scripts/gemini-canvas-browser-pool-fetch-page.mjs`. The execution owner keeps
request mode selection, page switching, fallback routing, and capture cleanup;
the new 144-effective-line owner handles the browser-context request, bounded
stream read, response projection, and base64 conversion. The execution owner is
439 effective lines, below the 500-line acceptable limit. The shared body owner
is 135 effective lines.

The pre-change measurement for this continuation was 76 effective lines for the
body owner, 137 for fetch-page, 168 for no-key fetch, 234 for payload, and 343
for preview. Current measurements are 135, 144, 175, 241, and 350 respectively;
each remains below 500 lines.

The 2026-09-24 full browser-pool run passed 945/945 tests with no failures or
skips. Its full-suite command was:

```powershell
$files = Get-ChildItem .\scripts\tests\gemini-canvas-browser-pool*.test.mjs -File |
  Select-Object -ExpandProperty FullName
node --test $files
```

The current continuation's focused Node suites pass 155/155 tests across 11
files, including zero-declared-length body validation, bodyless 304 handling,
stream cancellation, and reader/page/capture/timer cleanup. The exact focused
command appears below.

The current effective-line checker tests pass 31/31. The adoption ratchet scans
2,502 files with no governed source over 700 lines. The nested-worker package
contract passes 1/1 with the extracted owners included. The focused tests use
fixtures and an offline browser; they do not claim a real Gemini provider or
packaged runtime acceptance.

## Focused evidence

The body adapter, network-capture, fetch-operation, fetch-page-music, no-key
fetch, payload extraction, preview, network stop/registration, fetch execution and
proxy-launch suites pass together:

```powershell
node --test `
  scripts/tests/gemini-canvas-browser-pool-body.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.network-capture.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.fetch-operation.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.fetch-page-music.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.network-registration.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.payload.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.no-key-fetch.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.fetch-execution.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.preview.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.network-stop.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.proxy-launch.test.mjs
```

The package contract includes the new body and fetch-page owners, so nested
release packaging cannot silently omit them. `node --check` passes for all 16
changed JavaScript modules and test files. `git diff --check` and the Neuro
development-standard contract pass for this continuation.

## Remaining risk

Playwright exposes only whole-body `text()`/`body()` reads here. Missing or
malformed lengths now fail closed, but an understated in-range `Content-Length`
can still cause the complete body to allocate before the post-read check rejects
it; the bounded page-context fallback has the same limitation. A stream reader can
also receive one oversized chunk before cancelling. These checks do not prove a
hard browser or Node heap cap, and they do not close the separate CDP
`Network.getResponseBody` read. Connected-client per-response retention and
frame admission are tracked in separate follow-up records; aggregate memory,
S06/S18 and shared release/runtime validation remain open.
