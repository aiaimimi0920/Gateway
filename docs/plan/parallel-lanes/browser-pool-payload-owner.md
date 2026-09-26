# Browser pool payload owner ownership

Owner: resumed coordinator.

Accepted at 2026-09-14T18:03:41.737Z. Entry 3920 -> 3732 effective lines.

Moved extractAudioBytes, extractImageBytes and downloadBinaryViaNavigation together into a 189-effective-line named-export module. Same file reader and URL/MIME helper identities, callback bodies, error schemas, call sites and temporary-page finally cleanup are preserved. Twenty-five added contracts cover captured-byte bypass, credentials and multi-chunk encoding, distinct media validation, download/navigation precedence and cleanup across failures.

Fresh paired full Node suites pass 574/574 with identical identities/warnings and no skips; paired nested package passes 1/1 with exactly the new module path added. Independent projected review found no introduced defect. Real temporary download files are cleaned within checked temp-root paths; VM browser callbacks and page stubs do not claim real-provider execution.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 3732 effective lines.
- scripts/gemini-canvas-browser-pool-payload.mjs: 189 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 166 effective lines.
- scripts/tests/gemini-canvas-browser-pool.payload.test.mjs: 151 effective lines.
- scripts/tests/gemini-canvas-browser-pool.payload-fixtures.mjs: 72 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 195 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-payload-owner/scope.json.
SHA-256: 2b2d13030d817cf96af4d304ad2bbf92e2009d228234115e36664e74d319769e.
Predecessor: 591b029f93f7db10a8750027bcee4a8e185b5819cbf76529bb5d41b1f278759d.
Union 790; unchanged neighbors 787; web assets preserved.
Strict: 1946 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Extract buildTtsDiagnostics, listenControlCandidates, clickListenControlWithFallback and runTtsOperation together. Preserve fixture bypass before entry access, injected initialization order, capture stop in finally, Listen fallback, non-audio retry and error/result schemas. Use the shared payload module for bytes.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.

[Checkpoint report](../../status/2026-09-15-browser-pool-payload-owner.md).
