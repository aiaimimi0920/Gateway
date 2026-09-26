# Browser-pool connected-client response retention

Status: **hardening_green for this WebSocket owner**, 2026-09-25 UTC.
Unknown-length native reads, full S18, provider, packaged-runtime, UI, Docker,
and release acceptance remain open.

## Ownership and bounds

`gemini-canvas-browser-pool-connected-client.mjs` now caps each pending response
at the shared 4 MiB UTF-8 text-body budget. It counts decoded string bytes before
retaining each chunk. Overflow rejects with HTTP 413 and
`gemini_canvas_browser_body_too_large`, clears retained strings and the byte
counter, removes the pending request, clears its deadline, and ignores late
frames. On normal close, the body is joined once and chunk references are cleared.

The owner also retains at most 1,024 non-empty chunk strings per response;
empty chunks are ignored. The next chunk rejects with HTTP 413 and
`gemini_canvas_connected_client_chunk_limit` and follows the same cleanup path.
The fetch owner preserves 413 failures so the invocation dispatcher returns the
original status and code. Existing authentication, message attribution, body
ordering, timeout and disconnect behavior remain covered.

The connected-client owner moved from 217 to 263 effective lines, its test file
from 240 to 301, the body-limit owner from 69 to 76, and fetch execution from 436
to 439. No affected file exceeds 500 effective lines.

The budget applies per pending response. At this checkpoint, a WebSocket frame
was fully received and JSON-parsed before this owner measured its `data` field.
The follow-up frame-admission gate now checks application message bytes before
JSON parsing and sets the same per-frame `maxPayload` on the WebSocket server;
see the [frame-bounds record](2026-09-25-browser-pool-connected-client-frame-bounds.md).
Neither bound establishes a global concurrent-response or process-heap budget.

## Verification

Commands:

    node --test scripts/tests/gemini-canvas-browser-pool.connected-client.test.mjs scripts/tests/gemini-canvas-browser-pool.fetch-execution.test.mjs scripts/tests/gemini-canvas-browser-pool-body.test.mjs
    python -m unittest discover -s tests/python -p test_gateway_nested_worker_package_contract.py
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts

Focused tests pass **25/25**, the nested-worker package contract passes **1/1**,
effective-line checker tests pass **31/31**, and the ratchet scans **2,502** files
with no governed source above 700 effective lines. The Neuro development-standard
contract passes; Gateway `git diff --check`, UTF-8/BOM, and trailing-whitespace
checks also pass for this change. The full browser-pool, provider, runtime, Docker,
UI, and release suites were not rerun for this bounded retention change.
