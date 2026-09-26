# Browser pool composer owner checkpoint

Accepted at 2026-09-14T17:20:26.197Z. Entry 4631 -> 4531 effective lines.

Moved submitPrompt, tryClickSendButton and their shared candidate builder into a 99-effective-line named-export module. Two disjoint source blocks are preserved exactly apart from export keywords; root remainder, call sites, Chinese selectors and serialized DOM callbacks are unchanged. Eleven added contracts cover native input events, contenteditable typing, keyboard and button fallback, candidate ordering and timeout caps.

Fresh paired full Node suites pass 511/511 with identical identities/warnings and no skips; paired nested package passes 1/1 with exactly the new module path added. Independent projected review found no introduced defect. VM DOM fixtures do not claim real-browser/provider or non-host OS E2E.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 4531 effective lines.
- scripts/gemini-canvas-browser-pool-composer.mjs: 99 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 154 effective lines.
- scripts/tests/gemini-canvas-browser-pool.composer.test.mjs: 70 effective lines.
- scripts/tests/gemini-canvas-browser-pool.composer-fixtures.mjs: 48 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 192 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-composer-owner/scope.json.
SHA-256: 4b05eb855c90855396d5213660e36231017f24e7e6c236476345cb7f5bc643b4.
Predecessor: 4b20d93e38464f65cc51b39ad78e34279d7621c490e300ceefa56f9554689443.
Union 783; unchanged neighbors 780; web assets preserved.
Strict: 1939 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Extract cohesive media asset selection from snapshot, capture and invoke contract into a bounded owner. Preserve audio/image/video candidate precedence and keep byte download/transport separate.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
