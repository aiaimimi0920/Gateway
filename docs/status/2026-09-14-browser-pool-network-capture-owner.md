# Browser pool network capture owner checkpoint

Accepted at 2026-09-14T15:33:48.446Z. Entry 5593 -> 5311 effective lines.

Moved the complete network capture function into a 301-effective-line owner with six direct imports and eighteen injected helpers. All call sites and remaining entry bytes are preserved by inverse reconstruction. Twenty direct contracts preserve listener, bounds, filtering, deferred response/media and Cookie behavior. Independent review found no introduced extraction defect; late writes after stop require a separate behavioral fix.

Fresh paired full Node suites pass 431/431 with identical identities/warnings and no skips; paired nested package passes 1/1 with exactly one module path added.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 5311 effective lines.
- scripts/gemini-canvas-browser-pool-network-capture.mjs: 301 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 134 effective lines.
- scripts/tests/gemini-canvas-browser-pool.network-capture.test.mjs: 132 effective lines.
- scripts/tests/gemini-canvas-browser-pool.network-fixtures.mjs: 28 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 187 effective lines.

Evidence: target/effective-line-evidence/20260914-browser-pool-network-capture-owner/scope.json.
SHA-256: 08ff56abc36b64c6008c5a269813545f3dd0e39d0c2adb79a1bf1da276afc733.
Predecessor: b68255d6de09c2d338880d9127a48792c41f01402538540b631598cd720011dd.
Union 769; unchanged neighbors 766; web assets preserved.
Strict: 1925 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Reproduce and fix late response/body/Cookie writes after stop or shared-state page adoption in a separate red/green batch, then continue cohesive entry owner extraction.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
