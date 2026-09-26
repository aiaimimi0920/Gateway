# Browser pool HTTP media URL upgrade checkpoint

Accepted at 2026-09-14T16:05:11.584Z. Entry 5200 -> 5200 effective lines.

Corrected three overescaped hostname regex literals so HTTP assets on the three intended provider domains and their subdomains normalize to HTTPS and deduplicate with secure variants. Label boundaries and terminal anchors preserve lookalike-host rejection. Source outside the three lines, root, fixtures and package contract are unchanged. Owner remains 115 effective lines; new contracts have 38.

Fresh baseline: two preservation groups pass and four upgrade/dedupe regressions fail. Final regressions pass 6/6; full Node passes 458/458 with no skips or warning drift. Nested package passes 1/1. Tests cover host boundaries, nested subdomains, case, ports, signed query/fragment bytes, unrelated schemes and first-result media metadata.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 5200 effective lines.
- scripts/gemini-canvas-browser-pool-media-urls.mjs: 115 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 144 effective lines.
- scripts/tests/gemini-canvas-browser-pool.media-upgrade.test.mjs: 38 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 188 effective lines.

Evidence: target/effective-line-evidence/20260914-browser-pool-media-http-upgrade/scope.json.
SHA-256: 8994d5c80f7b15387c41c0a9762f776a4749db8e94ab7c77fb9df7c381b447a8.
Predecessor: 3a2b2cbf6180e830660bdbdd1b4e7a3db9c25f69957a7f051c9057aac4edc5d9.
Union 773; unchanged neighbors 772; web assets preserved.
Strict: 1929 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Extract the complete program snapshot collector while preserving its self-contained page.evaluate closure and host-side handle parsing.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
