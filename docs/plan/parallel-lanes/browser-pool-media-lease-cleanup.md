# Browser pool media lease startup cleanup ownership

Owner: resumed coordinator.

Accepted at 2026-09-14T21:13:16.211Z. Entry 2211 -> 2209 effective lines.

The existing media lease try/finally now begins before program URL resolution,
page reset and capture startup. Previously, failures in those stages skipped owned
page cleanup. Capture is nullable until acquired; attached pages remain open and
acquired capture stops before owned-page close. Reusing the normalized base URL
keeps this narrow repair at 2209 effective lines, two fewer than before.

Negative proof: 25 new contracts pass 22/25 before the fix; exactly three owned-page
startup cases fail with expected close count 1, actual 0. Final Node passes 745/745,
retaining all 720 prior identities/warnings and passing all 25 new contracts. Native
root tests cover invalid/unsupported inputs and three complete fixture schemas.
The VM harness retains real capture listeners and verifies error identity, cleanup
order, late events, timeout and resume behavior. Nested package 1/1 remains unchanged.
Independent review found no introduced defect; real navigation/provider output is
not established by these isolated lifecycle tests.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 2209 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 184 effective lines.
- scripts/tests/gemini-canvas-browser-pool.media-operation.test.mjs: 96 effective lines.
- scripts/tests/gemini-canvas-browser-pool.media-operation-fixtures.mjs: 54 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 202 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-media-lease-cleanup/scope.json.
SHA-256: b687d93bbcdcb6af74387b0064a2857682b3430412a364e551e8f0296de52018.
Predecessor: 7eae53bb1afe4af4651c92cfbde3e68594d51ad2476189f51ecdd7488708f588.
Union 810; unchanged neighbors 809; web assets preserved.
Strict: 1966 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Prepare full media-polling contracts, then extract its 460-line polling/timeout
block. A complete in-memory owner draft measures 497 effective lines, including
seven operation inputs, fourteen injected dependencies and same-source imports.
No owner is written or accepted yet. Cover video retries/provider gates/player
probes/music settlement, and keep the awaited call inside the repaired finally.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.

[Checkpoint report](../../status/2026-09-15-browser-pool-media-lease-cleanup.md).
