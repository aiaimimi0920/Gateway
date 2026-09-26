# LumaLabs browser operation owner

## Structural change

The browser-side LumaLabs protocol was extracted from
scripts/lumalabs-browser-worker.mjs into
scripts/lumalabs-session/browser-operation.mjs. The extracted function remains
self-contained for Playwright's page.evaluate serialization. Browser discovery,
context creation, cookies, board navigation, stdin parsing and process response
stay in the entrypoint.

The exact callback body was projected from the saved pre-change source. The only
production entrypoint changes are the new import and replacement of the inline
callback with `page.evaluate(executeLumaOperation, args)`. No protocol branch,
URL, header, timeout, retry, SSE parser, action response or error mapping was
rewritten.

Effective lines are now:

- Entry worker: 631 -> 195.
- Browser operation owner: 438.
- Focused operation tests: 128.

The new owner remains below the 500-line preferred acceptance ceiling. All new
files are UTF-8 without BOM. No formatter is defined for these ESM workers, so
the extracted body retains its source formatting.

## Behavior proof

The focused suite loads the exported function in a fresh VM browser context using
`toString()`, so Node imports and outer closures cannot hide a serialization bug.
Seven cases pass against both the saved original callback and the extracted owner:

- signature failure contract and timer cleanup;
- deadline abort mapping;
- split SSE chunks, completed artifact selection and reader cancellation;
- exact menu action discovery without mutating caller action input;
- menu failure fallback and action error metadata;
- missing signature token short-circuit;
- missing output id action-stage error.

The fixture checks request order, credentials, authorization header, action body,
artifact URL, error stage/status and timer release without contacting LumaLabs or
using credentials. This is an offline serialization/protocol regression suite,
not provider runtime validation.

Fresh commands from Gateway, with rtk, all exit zero:

- node --check scripts/lumalabs-browser-worker.mjs.
- node --check scripts/lumalabs-session/browser-operation.mjs.
- node --test scripts/tests/lumalabs-browser-operation.test.mjs: 7 passed.
- The same test against the saved original callback: 7 passed.
- npm run test:effective-lines --prefix scripts: 19 passed.
- npm run check:effective-lines --prefix scripts: ratchet passed.
- Python nested worker package contract: 1 passed.
- Gateway and Neuro git diff --check: independently passed.

The ratchet now scans 2256 files: 10 above 1500, 18 between 701 and 1500,
25 between 501 and 700. This clears the LumaLabs soft debt entry. No checker
policy, baseline, exception, package manifest or release directory changed.

## Remaining risks

The worker's pre-existing process.exit response path, browser close handling,
provider timeout semantics and real browser/SSE behavior were not changed in this
structural extraction. Existing path, secret, service availability and provider
contract assumptions remain. No real LumaLabs session, live credential, network
call, native build or release package was used.

Full strict closure and integrated release acceptance remain open. S06 Rust/Gemini
and the final native build window remain reserved pending a transfer receipt.
Existing dirty/staged work was preserved; no commit, push, sibling source, live
service or release/Gateway content was changed.
