# Browser pool program snapshot owner checkpoint

Accepted at 2026-09-14T16:17:19.511Z. Entry 5200 -> 5095 effective lines.

Moved the complete program snapshot collector into a 107-effective-line owner, preserving the self-contained browser callback and host-side handle parser boundary. Nine contracts execute serialized callback source in an isolated VM to verify caps, filtering, metadata, serialization fallback, host hint aggregation and failure propagation. Remaining root bytes and all call sites are preserved.

Fresh paired full Node suites pass 467/467 with identical identities/warnings and no skips; paired nested package passes 1/1 with exactly the new owner path added. Independent projection review found no introduced defect. VM/contract checks do not claim real-browser or provider E2E validation.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 5095 effective lines.
- scripts/gemini-canvas-browser-pool-program-snapshot.mjs: 107 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 145 effective lines.
- scripts/tests/gemini-canvas-browser-pool.program-snapshot.test.mjs: 91 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 189 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-program-snapshot-owner/scope.json.
SHA-256: 1714ea1bf2d72f0a99743feb44d5be01edf0fd1ec6730bc6e5649ca0b128156a.
Predecessor: 8994d5c80f7b15387c41c0a9762f776a4749db8e94ab7c77fb9df7c381b447a8.
Union 775; unchanged neighbors 772; web assets preserved.
Strict: 1931 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Extract pair ranking/canonicalization and program handle state assembly together, keeping the intervening proxy discovery/preview decisions in the root. Preserve navigation dependency injection and selection precedence with paired contracts.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
