# Browser pool video UI owner checkpoint

Accepted at 2026-09-14T17:47:39.145Z. Entry 4219 -> 3920 effective lines.

Moved four complete video chooser/create/template functions into a 298-effective-line named-export module. The only import is the shared composer sender; root remainder, localized matchers, serialized browser callbacks and all callers are preserved. Image-only Canvas exit and TTS behavior remain separate. Sixteen added contracts protect visibility/click fallback, metadata errors, candidate order/clamping, repeated native/synthetic clicks, bounded diagnostics and create/send behavior.

Fresh paired full Node suites pass 549/549 with identical identities/warnings and no skips; paired nested package passes 1/1 with exactly the new module path added. Independent source review found no introduced regression. Root fixture execution imports the real owner and verifies its wiring; isolated VM tests do not claim real-browser/provider E2E.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 3920 effective lines.
- scripts/gemini-canvas-browser-pool-video-ui.mjs: 298 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 163 effective lines.
- scripts/tests/gemini-canvas-browser-pool.video-ui.test.mjs: 152 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 194 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-video-ui-owner/scope.json.
SHA-256: 591b029f93f7db10a8750027bcee4a8e185b5819cbf76529bb5d41b1f278759d.
Predecessor: 18bfcc3d06abdb3f40f3e93c18c9a48c13f289f38ae3d5270d136681af8bda46.
Union 787; unchanged neighbors 784; web assets preserved.
Strict: 1943 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Read and measure the complete extractAudioBytes, extractImageBytes and downloadBinaryViaNavigation block before collectButtonSnapshot. Preserve distinct errors, captured-byte fast paths, browser credentials/encoding, download precedence and temporary-page finally cleanup. Keep selection separate, then tackle the TTS execution block with lifecycle contracts.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
