# Browser pool Bootstrap polling owner checkpoint

Accepted at 2026-09-14T20:34:41.869Z. Entry 2682 -> 2634 effective lines.

The 57-line polling phase moved into a 69-line owner. Current page/state, initial
snapshot and the existing merge callback are explicit inputs; root awaits the result
inside its original try/finally. The original Date.now calls, 1800 ms cadence,
12000 ms music stabilization, 15000 ms concrete-handle fallback and timestamp
persistence remain unchanged. Bootstrap decreases from 479 to 426 effective lines.

Paired full Node 720/720 with identical identities/warnings and zero skips; paired
nested package 1/1 checks the new module bytes, manifest and checksums. Twenty-five
polling contracts plus three full-operation polling-failure cleanup cases were added.
The owner is prepared from original inline source before baseline; after integration
the same VM harness supplies virtual time without adding a production clock API.
Inverse projection restores the original loop and root exactly.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 2634 effective lines.
- scripts/gemini-canvas-browser-pool-bootstrap-polling.mjs: 69 effective lines.
- scripts/tests/gemini-canvas-browser-pool.bootstrap-fixtures.mjs: 121 effective lines.
- scripts/tests/gemini-canvas-browser-pool.bootstrap-polling.test.mjs: 161 effective lines.
- scripts/tests/gemini-canvas-browser-pool.bootstrap.test.mjs: 195 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 201 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-bootstrap-polling/scope.json.
SHA-256: 5847a643a581c9866564ab58bcd9b7b33df252126db17d17a4890ca11704ddeb.
Predecessor: 07a563da769e2d179d72d8803ad1b800eaacc11823d6513cf340a15113437b3d.
Union 807; unchanged neighbors 805; web assets preserved.
Strict: 1963 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Move the complete Bootstrap execution and its two configuration helpers into
a cohesive owner. An in-memory draft measures 494 effective lines, including readable
imports and 26 one-name-per-line injected dependencies. Keep hasConcreteProgramHandleState
outside because the polling factory still needs it. This draft is not yet written
or accepted; verify exact bodies, initialization, packaging and the full paired suite.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
