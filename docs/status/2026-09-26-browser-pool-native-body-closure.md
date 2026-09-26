# Browser-pool native body admission and acceptance retry

Status: hardening_green for the main browser-pool response owner and proxy-launch
response transport. S06/S18 and final release acceptance remain in_progress.
The immutable Windows r2 package is unchanged and predates this source change.

## Implementation

The pool's main response listener now uses the existing Chromium CDP capture
owner. Content-Length is no longer its authority for admitting a whole body.
The native owner accounts for decoded Network.dataReceived bytes, waits for
loadingFinished, and admits the body before issuing Network.getResponseBody.
Unknown declared lengths remain usable when Chromium's observed decoded size is
known. Compressed responses exceeding the text budget fail before body retrieval.

The shared native owner now exposes binary bytes without UTF-8 conversion, memoizes
one protocol read across body/text accessors, and supplies request headers. Text
keeps its 4 MiB ceiling; image/audio/octet-stream bytes have a configurable ceiling
up to 16 MiB. Eight native reads and a conservative escaped-envelope budget bound
active protocol reads. Binary-enabled callers reserve at most 128 MiB plus framing
per envelope and twice that across concurrent reads. These are admission and
retention bounds, not a claim about the browser process's total heap.

Oversized metadata-only media does not abort capture. A later request to read its
body fails closed. Stopped readers cannot issue new native body requests. The
existing ChildSession send timeout and dispose rejection remain responsible for
settling outstanding protocol commands; no unbounded Promise.race fallback was
added. Sibling pages are excluded by a pageOnly capture option, preserving the
pool's page-level ownership. The program-handle worker keeps its existing default
context-wide mode for adoption and popups.

Bootstrap, fetch, media, debug, text and TTS execution await capture readiness and
cleanup. Proxy launch uses the same native response transport and suppresses late
network event publication after cleanup. Failed listener detachment still attempts
all cleanup and preserves the first error.

## Evidence

Evidence root: `target/release-evidence/closure-native-body/`.

- Before changes: selected existing body/capture tests, 71 passed.
- After changes: `adjacent-final.log` / `.json`, 1,274 passed across 93 test files,
  zero failures/skips. This includes binary, compressed-size admission, late reads,
  metadata-only large video, registration failure and lifecycle coverage.
- A subsequent proxy cleanup correction ensures partial registration rolls back
  and a listener-detach error cannot bypass native transport cleanup. Its changed
  owner and adjacent binding tests pass 21/21 in `proxy-cleanup-final.log`, including
  two new failure-path regressions. The whole 1,274-case group was not repeated
  after that local correction; unchanged owners reuse their matching evidence.
- `browser-proof.mjs` / `.json`: five real Edge loopback scenarios pass: same-context
  sibling page isolation, unknown-length program response, exact binary bytes,
  gzip expansion rejection without a native body command, and owned listener cleanup.
  Only two admitted native body reads occur. The driver closes its browser,
  context and local HTTP server; no real provider account or credentials are used.
- `package-contract.log`: nested-worker packaged import contract, 1/1 passed.
- Official checker tests: 31/31 passed. See the final ratchet/strict receipts for
  current source measurements. Worker scripts have no configured formatter;
  importing them in the focused suites covers their syntax.
- `after-lines.json` records the modified files' counts and encoding checks.
  Native response owner: 181 to 208; pool capture: 333 to 348; body adapter:
  136 to 138; proxy launch: 235 to 241; CDP session owner: 279 to 282.
  The largest affected owner is fetch execution at 440 effective lines.
  The new binary suite is 66 effective lines. No BOM/trailing whitespace was found.

The new code invalidates whole-source provenance reuse of r2 for current inputs.
Do not overwrite r2 or copy its binaries into a supposedly current-source package.
After the remaining code gates are closed, build a new immutable candidate and
repeat its package/runtime/UI/Docker acceptance gates.

## Docker retry

Evidence root: `target/release-evidence/closure-docker-20260926T040826228Z/`.

The retry retained the official Dockerfile and release profile, one Cargo job,
incremental disabled, existing caches and fresh production dependency audits.
No optimization level or global WSL configuration was changed. The build again
reached Rust compilation, then Docker version and persistent 4200 health/readiness
probes timed out. The exact owned build client was cancelled after matching PID
33776 and its unique image tag. Its receipt records failure, not image completion.

After cancellation, 4200 healthz and readyz recovered to HTTP 200 without a restart.
deploy/.env retained its original SHA-256. A bounded dmesg query found no OOM rows;
OOM remains unproven. No Compose verification ran, and no successful Docker image
or final per-object inventory is claimed for this attempt.

An explicit user decision was requested before backing up .wslconfig, changing
8 GB memory / 2 GB swap to 12 GB / 8 GB, and restarting WSL/Docker. The restart
affects all WSL sessions and Docker containers. No answer or global mutation is
recorded in this checkpoint. Do not launch another identical unbounded retry.

## Still open

- S18 navigation-download fallback and standalone image-edit broad-capture still
  have distinct response-body admission paths. The navigation download also reads
  a completed file before its actual-size check. This batch does not close them.
- S06 reserved Rust/Gemini owner-level acceptance remains separate; no reserved
  Rust source was changed.
- Docker release image and isolated Compose acceptance need a successful retry
  and post-run resource/environment checks.
- Current-source release packaging, integrated desktop workflows and real-provider
  acceptance still require their own results. The loopback tests above do not
  substitute for authenticated provider functionality or full visual testing.
