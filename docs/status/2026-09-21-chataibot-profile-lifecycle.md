# ChatAIBot profile lifecycle owner

## Change

The coordinator extracted profile cloning and recursive copying from the worker
into scripts/chataibot-session/profile.mjs. The entrypoint remains responsible
for browser selection, browser context, credential output and response protocol.

Two confirmed cleanup defects were repaired:

- Immediate process.exit skipped main's finally on both success and failure.
  writeWorkerResult now sets process.exitCode and returns, allowing context close
  and clone removal before natural process exit, with the same JSON and exit code.
- A clone-copy failure happened before the caller received its temporary path.
  The profile owner now removes its partially populated directory before rethrowing
  the original error. The caller takes ownership only after a successful return.

Effective lines: entry 405 -> 327; profile owner 86; lifecycle tests 66;
browser fixture 27; module loader 10; filesystem failure fixture 8. New small
fixtures are distinct ESM loader boundaries, not production forwarding layers.
All inspected source files are UTF-8 without BOM. Existing formatting is retained;
there is no official formatter script for these worker modules.

## Regression evidence

Tests execute the real worker entrypoint in isolated Node subprocesses, replacing
Playwright with a synthetic browser. Each fixture owns its source profile, temp
directory and event log; no real browser, provider or credential store is opened.

Before the production change, all five corrected baseline cases failed:
success/probe/close paths skipped browser close, and launch/copy failures leaked
the temporary clone. An initial directory-as-file copy fixture was unsuitable on
Windows because EPERM is deliberately ignored; the corrected baseline injected
EIO and reproduced all five cleanup failures before production edits.

After the fix, all five cases passed. They assert one JSON response, exit status,
response fields, awaited asynchronous close, an empty temporary root and unchanged
source Preferences. The final copy fixture was strengthened after review: it now
copies Local State before failing on Preferences, proving partial-copy cleanup;
it also checks that source Local State is unchanged. Those final five cases passed.
The final strengthened fixture was not rerun against the old production snapshot.

Fresh verification from Gateway, using rtk:

- node --test scripts/tests/chataibot-session-lifecycle.test.mjs
  scripts/tests/chataibot-session-probe.test.mjs: 10 passed before the final
  fixture strengthening; lifecycle-only rerun afterward: 5 passed.
- node --check scripts/chataibot-session-worker.mjs: passed.
- node --check scripts/chataibot-session/profile.mjs: passed.
- npm run test:effective-lines --prefix scripts: 19 passed.
- npm run check:effective-lines --prefix scripts: passed, including a final
  rerun after the fixture change.
- python -m unittest discover -s tests/python
  -p test_gateway_nested_worker_package_contract.py -v: 1 passed.
- Gateway and Neuro git diff --check: independently passed.

Independent read-only review confirmed ownership transfer, JSON/exit behavior,
fixture isolation and the cleanup fix. It identified the initial copy-failure
coverage limitation, addressed by the final fixture change above.

## Limits and continuation

Natural process exit now waits for event-loop work and browser close. A stuck close
has no new deadline; no universal bounded-exit claim is made. The generic Rust
producer supervisor has timeout/tree-kill/reap handling, but no direct ChatAIBot
worker caller was found, so that protection cannot be assumed for this script.

Profile path trust, symlink traversal, TOCTOU, unbounded profile copies/stdin,
quota/IndexedDB deadlines and silent rm failure remain outside this batch.
Synthetic tests prove cleanup execution, not real Chromium or locked-file cleanup.

The checker scans 2254 files with 10 above 1500, 18 between 701 and 1500 and 26
between 501 and 700. Those debt totals did not change in this lifecycle batch.
Full strict closure, integrated runtime validation and native release remain open.
The S06/final native build ownership reservation remains unchanged.

Only the worker, its profile module, focused fixture/tests and these coordination
records were changed. Existing dirty/staged work was retained. No commit, push,
live service, real profile, sibling source or release/Gateway content was changed.
