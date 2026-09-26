# Udio capture utility owner

This checkpoint splits the unreserved Udio capture utility by responsibility.
The existing CLI entry point and JSON failure/success protocol remain intact;
the coordinator did not modify the reserved Udio manual-browser or production
worker source scopes, S06 Rust/Gemini files, browser payloads, or release
inputs.

## Scope and ownership

`scripts/udio-capture-next-generate.mjs` remains the executable entry point and
keeps browser connection, page selection, request-finished/request-failed
listeners, polling order, output/status writes, and final browser/monitor
cleanup. Its pure input and file helpers now live in
`scripts/udio-capture-next-generate.input.mjs`; CDP target monitoring and the
page fetch/XHR hook now live in
`scripts/udio-capture-next-generate.browser.mjs`.

The new owners are imported with explicit ESM paths. No browser is launched by
the focused tests. The packager's recursive `scripts` copy includes both owner
modules, while the entry filename and stdin/stdout contract remain unchanged.

## Size result

The authoritative effective-line scan changed:

- entry point: 549 -> 228 effective lines (598 -> 244 physical lines);
- input/file owner: 107 effective lines (120 physical lines);
- browser/CDP owner: 231 effective lines (252 physical lines);
- focused contract test: 171 effective lines (186 physical lines);
- soft-limit inventory: 12 -> 11 files;
- current scan: 2,335 files, with 6 above 1,500, 8 in 701-1,500, and 14
  total files above 700 including the reserved S06 hubs and browser-profile
  payloads.

All new and changed owners are below 500 effective lines. No baseline,
checker policy, exception record, runtime profile, release artifact, or live
provider state was changed.

## Focused verification

- New capture utility contracts: 4/4 passed.
- Combined Udio capture, manual-browser, and production-worker Node suites:
  31/31 passed.
- Node syntax checks passed for the entry, both owners, and the new test.

