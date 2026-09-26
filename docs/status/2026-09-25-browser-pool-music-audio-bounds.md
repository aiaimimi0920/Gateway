# Browser-pool no-key music audio retention

Status: **hardening_green for this WebSocket owner**, 2026-09-25 UTC.
Full S18, provider, packaged-runtime, UI, Docker and release acceptance remain open.

## Ownership and bound

The page-context and preview-frame no-key music paths now share the
scripts/gemini-canvas-browser-pool-no-key-music.mjs owner. The two previous
311-effective-line implementations became 10- and 9-effective-line adapters.
The 377-effective-line shared owner contains one self-contained Playwright
evaluate callback: splitting its browser-side state machine into imported
helpers would lose the closure-free serialization contract. This reduces the two
production owners from 622 to 396 effective lines.

The callback receives the existing 16 MiB binary limit explicitly as an
evaluate argument. It counts decoded Base64 bytes before calling atob; an
oversized chunk returns HTTP 413 with
gemini_canvas_browser_body_too_large, clears retained chunks, closes the socket,
clears both timers, and does not retry the alternate empty-key URL. Successful
audio, setup ordering, event bounds, timeout behavior and page/preview error
names remain covered by the shared variant suite.

The fetch owners preserve the 413 status and body-limit error code. The nested
worker package contract includes the new module.

## Verification

Validation commands:

    node --test scripts/tests/gemini-canvas-browser-pool.no-key-music.test.mjs scripts/tests/gemini-canvas-browser-pool.fetch-page-music.test.mjs scripts/tests/gemini-canvas-browser-pool.fetch-preview.test.mjs
    python -m unittest discover -s tests/python -p test_gateway_nested_worker_package_contract.py
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts

The focused Node suites pass **74/74**, the package contract passes **1/1**, the
effective-line checker tests pass **31/31**, and the ratchet scans 2,502 files
with no governed source above 700 effective lines. The broader browser-pool,
provider, native-capture, packaged-runtime, UI, Docker and release suites were not
rerun for this bounded owner change.
