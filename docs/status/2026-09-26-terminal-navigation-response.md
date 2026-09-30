# Terminal navigation response correction

Status: corrected and verified. This supplements the navigation/broad body
checkpoint. It does not close the shared release acceptance window.

## Defect and correction

Navigation capture previously skipped every 300-399 status except 304. A terminal
300 or 302 without a redirect therefore never resolved the body promise and failed
with a capture timeout, despite a valid document body being available.

The native owner now exposes `isRedirect()` from its actual CDP redirect transition.
Navigation skips only those intermediate responses. Terminal 3xx responses retain
their status, URL and admitted body. Actual redirect chains still return the final
document. No request is correlated by URL and no body-size check is bypassed.

This batch changes only these source/test files:

- `scripts/gemini-canvas-program-handle-response-capture.mjs`: 223 effective lines,
  unchanged count; adds the redirect transition flag to the synthetic response.
- `scripts/gemini-canvas-browser-pool-navigation-body.mjs`: 57 effective lines,
  unchanged count; consumes the flag instead of guessing from HTTP status.
- `scripts/tests/gemini-canvas-browser-pool.navigation-body.test.mjs`: 94 -> 103
  effective lines; terminal 300/302 regressions.

## Verification

- `target/terminal-3xx-before.log`: both new cases failed before the production
  change; nine existing cases passed. This reproduces the actual timeout boundary.
- `target/terminal-3xx-focused.log`: 80/80 passed across seven native response,
  navigation, payload and broad-capture test files.
- `target/terminal-3xx-browser-proof.json`: three real headless Edge loopback cases
  passed: terminal 300; terminal 302; followed 302 returning the final document.
  Each checks status, URL, body and temporary-page closure. The receipt records
  unchanged input hashes and completed browser/context/server cleanup.
- The initial browser proof had a fixture HTTP handler error on a browser auxiliary
  request. The handler now returns 404 for unrelated paths. This required no product
  change; the completed receipt above comes from the corrected proof.
- `target/terminal-3xx-node-full.log` and `.json`: complete CI Node worker file set,
  187 files, 2,084 passed, zero failed, one expected Windows skip for POSIX credential
  file permissions. `--test-concurrency=2` limits competition with the other task.
  The receipt contains pre-run hashes of worker source and test inputs and confirms
  `changedInputs: []` after the run. Checker tests are included in this complete set.
- `target/terminal-3xx-ratchet.json` and `target/terminal-3xx-strict.json`: exit 0.
  All three changed files are UTF-8 without BOM or trailing whitespace. No worker
  formatter is configured. Their imports/syntax were exercised by the tests.
- Gateway and Neuro working-tree and index `git diff --check` pass separately.

## Concurrent task boundary and remaining acceptance

The user confirmed another AI is actively optimizing Gateway. Its desktop
connection, Rust configuration/runtime, deployment, integration and README files
remain outside this batch. No changes were reverted or staged, no commit was made,
and the shared plan/board was not edited in this continuation. This uniquely named
checkpoint and ignored `target/terminal-3xx-*` evidence belong to this task.

No Cargo/Tauri/release build, persistent service change, WSL restart, Docker rebuild
or live provider call ran. The existing r2 release was not modified. A final
current-source candidate needs a stable source snapshot and a coordinated build
window after the other optimization task reaches its checkpoint. Docker/Compose
acceptance remains outstanding; the previously requested global WSL restart has
not received specific approval in this continuation.

The 2026-09-24 integration checkpoint already records eight Redis route cases and
splitter E2E as passed. Earlier 2026-09-23 notes listing them as untested are
historical; they are not newly rediscovered code defects. This report does not
claim those old results cover the other AI's current runtime/configuration changes.
The current full Node worker gate is now closed for the recorded hashes; overall
S06/S18 and release/runtime/UI/Docker acceptance remain open.
