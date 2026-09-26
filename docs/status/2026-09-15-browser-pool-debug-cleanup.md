# Browser pool debug capture cleanup checkpoint

Accepted at 2026-09-14T19:01:50.769Z. Entry 3135 -> 3138 effective lines.

Moved the existing success-only stop into one finally around acquired-capture work.
Reset and acquisition stay outside; disabled capture remains a no-op. Result fields
and diagnostic callbacks are preserved. The old root grows by exactly three effective
lines for the owning lifecycle boundary; mixing extraction into this behavior fix would
broaden verification. No package change or new owner is introduced.

Fresh negative proof: seven preservation cases pass and eight post-capture failures
leave two request listeners instead of the one inherited listener. After repair all
15 pass, including request/response/websocket listener identity and late-event checks.
Final full Node passes 642/642 without skips; the prior 627 identities/warnings remain.
Fresh package passes 1/1. Tests run the actual operation body in VM with the real
network capture module on EventEmitter; they do not establish browser/provider E2E.
The first fixture run used boolean true, disabling capture through string-only parsing.
Only the fixture default changed to string true; failed receipts and verified unchanged-
production recovery proof remain preserved. Independent review found no introduced
ownership/schema defect; hypothetical page.off failure can still mask a primary error.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 3138 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 177 effective lines.
- scripts/tests/gemini-canvas-browser-pool.debug-cleanup.test.mjs: 63 effective lines.
- scripts/tests/gemini-canvas-browser-pool.debug-fixtures.mjs: 50 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 197 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-debug-cleanup-v2/scope.json.
SHA-256: 7988674c594b8b053829106c9e6f30b65459b0a4a41065c53090d4a4b3cbf752.
Predecessor: 0640eca93b0ab06211f8ebdb6ebf62f713d302138f05c78781ed362553c9a10f.
Union 798; unchanged neighbors 797; web assets preserved.
Strict: 1954 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Extract debug execution and shared page/button snapshots into cohesive owners
under 500 effective lines, preserving the accepted finally boundary and existing
diagnostic schema. Exact bodies are already read; measure and add snapshot contracts
before extraction. Bootstrap still needs its full read and one adoption/finally boundary.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
