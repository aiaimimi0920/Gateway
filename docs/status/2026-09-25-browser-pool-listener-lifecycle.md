# Browser-pool listener lifecycle repair

Status: complete for listener acquisition and detachment, 2026-09-25 UTC.
Full S18 and product/release acceptance remain open.

## Failure and repair

`startNetworkCapture` registered three page listeners before returning its owner.
A registration exception left earlier listeners live without a returned cleanup
handle. The old `stop` also stopped detaching after the first `page.off` exception.

The owner now shares the exact listener bindings between registration and cleanup.
Startup failure revokes the lifetime, attempts every detachment and preserves the
original acquisition error. Normal stop likewise attempts all three detachments
before rethrowing its first cleanup error. External listeners remain untouched.
An in-flight response cannot publish after failed acquisition, including when the
state came from a previous page. A broken `off` implementation can still fail to
remove its own listener; the stopped flag revokes that callback and the error is
not hidden on normal stop.

Only `scripts/gemini-canvas-browser-pool-network-capture.mjs` changed in production.
Its parser, request/response callbacks, media fallback and WebSocket callback bytes
are exact through the registration boundary. The public `{ state, stop }` shape
and successful registration order are unchanged. The owner grows from **311 to
315 effective lines**; the new regression file has **46 effective lines**. The
three-element listener/error collections are bounded, and no asynchronous task,
new dependency or process is introduced.

## Focused evidence

Evidence root:
`target/effective-line-evidence/20260924-integration-closure/browser-pool-listener-lifecycle-20260925/`.

Two new regressions were run against the saved old implementation first:
**0 passed, 2 failed**. Both failures identify remaining owned listeners rather
than fixture/import errors. They exercise an actual in-flight response during
failed startup and a failing first detachment while other listeners remain.

The single post-fix run covers:

```powershell
rtk proxy node --test `
  scripts/tests/gemini-canvas-browser-pool.network-capture.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.network-stop.test.mjs `
  scripts/tests/gemini-canvas-browser-pool.network-registration.test.mjs
```

Result: **35 passed**, zero failed, skipped or cancelled. This includes the
existing 33 capture/stop cases and the two new regressions. Fresh Node imports
cover the changed module's syntax. The worker package has no declared formatter;
UTF-8/no-BOM and trailing-whitespace checks pass.

`rtk proxy npm run check:effective-lines --prefix scripts` exits **0**. The latest
scan contains **2,498 files**, including the same fourteen classified runtime
assets; all **2,484 governed source files are at most 500 effective lines**.
The S20 checker tests (31/31) and strict result are reused: checker,
policy/baseline/exception and test source bytes are unchanged. Later authority-prose
edits preserve the asserted documentation contracts. There was no full Node,
Cargo, provider-matrix, desktop or browser replay for this synchronous owner fix.

Hash comparison against `s20-migration-20260925/after.json` finds exactly one
changed protected input: this network owner. The new regression file is the only
new source row. The before-owner copy and failing logs remain available; no
source was overwritten from the older candidate tree.

## Next boundary

Browser-pool response text and image/audio body capture still call Playwright's
whole-body readers before trimming or retaining output. Their allocation and
retention limits need a separate change. Preserve URL-only media fallback and
per-page ownership: the pool supports independent captures, account-keyed busy
leases and attached CDP contexts. The standalone program-handle context-wide
transport must not be applied blindly to those lifetimes. No native allocation,
provider, packaged runtime, UI, Docker or release acceptance is claimed here.
