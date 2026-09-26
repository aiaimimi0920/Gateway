# Browser pool proxy capture parser binding checkpoint

Accepted at 2026-09-14T09:47:10.695Z. Entry 5874 -> 5876 effective lines.

Fixed two missing root parser bindings used by classifyHandlePairSurface. The Proxy Discovery owner now returns its existing WebSocket URL and target-domain parsers and the root receives them. Eight request/response regressions reproduced ReferenceError, including marker-only payloads with no handle pair. Existing parser bodies and capture callbacks are byte-preserved. Root grows by two minimal wiring lines, 5,874 to 5,876, to repair this oversized-file defect without mixing in capture restructuring; the owner is 75 effective lines and the new contract 65.

Focused regressions pass 0/8 before the fix and 8/8 afterward. Full final Node tests pass 360/360 without skips; package contract passes 1/1; checker tests pass 19/19. Source proof, UTF-8/no-BOM, syntax, ratchet and both Git checks pass. Strict remains expected exit 1: 1,918 scanned, 16 hard, 24 mandatory, 40 soft. This fix has focused red/green plus full final evidence, not paired full-suite baseline/final evidence.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 5876 effective lines.
- scripts/gemini-canvas-browser-pool-proxy-discovery.mjs: 75 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 116 effective lines.
- scripts/tests/gemini-canvas-browser-pool.proxy-capture-binding.test.mjs: 65 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 184 effective lines.

Evidence: target/effective-line-evidence/20260914-browser-pool-proxy-capture-binding/scope.json.
SHA-256: 6b05405dddbcee27d49452500177a2bfcce198d6d011e82fccce66acec259982.
Predecessor: d02f7ede39ba87f0584d2deb2eb6c22ffa31b0499e0581f02b93434d8f771433.
Union 762; unchanged neighbors 760; web assets preserved.
Strict: 1918 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Extract stateless program-handle parsing separately from capture metadata so early RPC/transport owners can import primitives while metadata classification stays downstream of Proxy Discovery. Keep escaped proxy metadata, URL admission, payload bounds and pre-try media reset cleanup explicit. S06 and all whole-plan release gates remain open.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
