# Browser pool debug and shared snapshot owners checkpoint

Accepted at 2026-09-14T19:30:58.895Z. Entry 3138 -> 2915 effective lines.

Moved runDebugOperation into a 160-effective-line factory owner and shared
collectButtonSnapshot/collectPageSnapshot into a 79-line pure module. Function bodies
change only by export declarations and factory indentation. The accepted finally,
diagnostic schemas, distinct snapshot limits and root call sites remain unchanged.
Pure ESM snapshot exports preserve early app/navigation/TTS dependency availability;
debug construction follows the capture/reset/operation-UI owners.

Paired full Node suites pass 655/655 with identical identities/warnings and no skips.
Paired nested package passes 1/1 with exactly two owner paths added. Thirteen new
contracts exercise serialized shared callbacks and an actual-root nonempty debug
composition without navigation/capture. Existing 15 debug cleanup contracts remain
unchanged and pass. The coordinator verified exact source reconstruction and dependency
review; no new lifecycle, initialization or schema defect was found.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 2915 effective lines.
- scripts/gemini-canvas-browser-pool-debug.mjs: 160 effective lines.
- scripts/gemini-canvas-browser-pool-page-snapshot.mjs: 79 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 179 effective lines.
- scripts/tests/gemini-canvas-browser-pool.page-snapshot.test.mjs: 103 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 199 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-debug-snapshot-owners/scope.json.
SHA-256: d6e4e6817368e7538ca0658f9a7bd6d1064961e64cfe732b5d1c53c5041f34a4.
Predecessor: 7988674c594b8b053829106c9e6f30b65459b0a4a41065c53090d4a4b3cbf752.
Union 801; unchanged neighbors 797; web assets preserved.
Strict: 1957 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Bootstrap full source is now read. Eleven identical snapshot-state merge blocks
occupy 176 effective lines. Add orchestration/state-order contracts, then consolidate
that seam before splitting larger phases. Preserve current capture.state identity,
handle/action/invoke ordering, one adoptActivePage boundary and one outer finally.
Final local result assembly is distinct. ensureProgramPage/ensureSharePage/ensureAppPage
navigate the existing Page; their inspected code does not replace entry.page.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
