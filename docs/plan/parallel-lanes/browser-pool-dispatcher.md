# Browser pool invocation dispatcher ownership

Owner: resumed coordinator.

Accepted at 2026-09-14T22:12:45.113Z. Entry 1584 -> 1382 effective lines.

The complete 221-line invocation function moved into a 243-line owner. Three
direct imports retain their original modules and sixteen dependencies are initialized
first or hoisted. Account scoping, context lease ownership, cookie policy, routing,
error envelopes and awaited authentication cleanup remain. Shared context state and
server lifecycle stay outside the dispatcher; exact inverse projection restores root.

Paired full Node 817/817 retains all 775 prior identities and adds 42 dispatcher
contracts with identical warnings and zero skips. Tests execute the actual function
body with controlled dependencies/time and isolated environment, covering held leases,
cookie policy, navigation, routing and error cleanup. The first prepared baseline
passed 816/817 because three URL assertions omitted an existing trailing slash; only
those expectations changed, with failed receipts and exact recovery proof preserved.
Paired package 1/1 protects owner bytes, manifest and checksums. Independent reviews
found no introduced projection or dispatcher-contract blocker.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 1382 effective lines.
- scripts/gemini-canvas-browser-pool-dispatcher.mjs: 243 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 185 effective lines.
- scripts/tests/gemini-canvas-browser-pool.dispatcher-fixtures.mjs: 58 effective lines.
- scripts/tests/gemini-canvas-browser-pool.dispatcher.test.mjs: 138 effective lines.
- scripts/tests/gemini-canvas-browser-pool.dispatcher-cookies.test.mjs: 50 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 205 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-dispatcher-v2/scope.json.
SHA-256: fac68825359ba74bb6f743faa7f3ae67ff5f901d1162dc701b7485dde1f4ef58.
Predecessor: c00b1dcb158e8d5b859660ca2afacebc61dde111245989132cc9bf13a6924a84.
Union 817; unchanged neighbors 814; web assets preserved.
Strict: 1973 scanned, 15 hard, 25 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Inspect the complete 588-line fetch runner and add direct page-restoration, capture,
preview/fixture and retry/fallback contracts before extracting its 118-line preview
branch. Dispatcher tests stub the fetch runner and do not prove its lifecycle.
No fetch owner or behavior change has been prepared in this checkpoint.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.

[Checkpoint report](../../status/2026-09-15-browser-pool-dispatcher.md).
