# Navigation and standalone broad-capture body admission

Status: owner-level hardening_green. S06/S18 integrated and release acceptance
remain open. This checkpoint supersedes the navigation/broad-capture code gaps
listed in `2026-09-26-browser-pool-native-body-closure.md`.

## Changes

- Navigation downloads read a file stream in 64 KiB chunks, with an inclusive
  endpoint of 16 MiB. At most one overflow byte is read, and the size is checked
  before retaining each chunk. File growth cannot bypass a pre-read stat check.
- Navigation fallback uses the native decoded-byte owner, attached before goto.
  Main-frame Document requests are selected by frame and request identity;
  redirects are ignored until the final response. Child targets are not configured
  for this mode. The body is captured eagerly before Chromium can evict it while
  the download-event wait is pending. Text remains limited to 4 MiB; non-text
  documents, including absent MIME metadata, retain the 16 MiB binary limit.
- Navigation native body/read/loading failures reject instead of becoming a
  successful empty body. A bounded capture timer and stop settle the result;
  existing goto/download wait timeouts remain separate phase timeouts, not a
  new promise of an absolute end-to-end deadline. Download events still win over
  navigation errors. Cleanup attempts both native stop and temporary page close.
- Standalone image-edit response capture now uses the same native owner, with
  request resource type retained. It awaits native readiness before navigation
  and native cleanup before closing the browser context. The old Page.text read
  path is removed. Native failure emits a capture-error event and suppresses
  incomplete response publication. Request/upload and WebSocket capture remain.
- The standalone legacy CDP RPC preview path also requires observed decoded
  bytes. Content-Length alone, including zero, cannot authorize a body command.
  Bodyless HTTP/method cases remain exempt from native reads.

## Fresh verification

All paths below are relative to Gateway.

- `target/navigation-broad-serialized.log`: 1,362 passed, zero failures/skips,
  100 browser-pool, program-handle and image-edit-broad test files. The command
  used Node 22 and `--test-concurrency=4`.
- The first unrestricted 100-file parallel run had one VM fixture initialization
  timeout at its existing 1,000 ms bound; 1,361 tests passed. The same complete set
  passed with bounded concurrency, without relaxing that timeout or assertions.
- `target/navigation-browser-proof.json` and `.mjs`: seven real headless Edge
  loopback cases passed, with three admitted native reads: redirect final status,
  URL and chunked text; main-document/child-frame isolation; gzip rejection before
  reading; exact download bytes; temporary-page cleanup; broad-capture response
  metadata/body; broad-capture gzip rejection before reading.
- Edge creates an additional empty-URL page during a download. A no-native-capture
  control reproduced the same extra page. Every page allocated by the navigation
  operation closed; the isolated proof closes its full context/browser/server.
  The production cleanup does not close unrelated context pages.
- `target/navigation-checker.log`: official checker tests 31/31 passed.
  `target/navigation-ratchet.json` and `target/navigation-strict.json`: passed.
- `target/navigation-package.log`: nested-worker package contract passed 1/1.
  `target/navigation-neuro-standard.log`: Neuro development-standard contract passed.
- All 11 affected JS source/test files pass `node --check`, UTF-8/no-BOM and
  trailing-whitespace checks. Worker scripts have no configured formatter.
- Gateway and Neuro working-tree and staged `git diff --check` passed separately.
  This task does not commit, reset or overwrite other worktree changes.

Effective lines: payload 241 -> 233; native response owner 208 -> 223;
broad capture 401 -> 406; broad runner 397. New navigation owner: 57;
new navigation and broad-native test suites: 94 and 73. All changed source/test
files are below 500, without exceptions or baseline relaxation.

## Remaining acceptance

The immutable Windows r2 package predates these changes and remains untouched.
No current-source release rebuild, final UI/provider workflow result, Docker image
or isolated Compose success is claimed here. S06 reserved Rust/Gemini acceptance
is not closed by these JavaScript tests. Authenticated provider testing remains
separate from the loopback proof.

Docker's repeated build stall and the pending global WSL resource/restart decision
remain documented in the prior checkpoint. No WSL mutation or identical Docker
build retry occurred in this batch. An explicit restart decision has been requested.

The current Gateway worktree also contains separately authored desktop connection,
configuration, runtime and deployment changes, and HEAD is now `d039d53`. Those
changes were preserved. Earlier whole-source r2 provenance cannot be reused for
this worktree. A new candidate must be built and accepted from a stable snapshot
after the integrated code gates are closed.
