# Browser pool conversation reset owner checkpoint

Accepted at 2026-09-14T09:36:15.751Z. Entry 5966 -> 5874 effective lines.

Extracted clickNewChat, resetConversation and dismissGeminiAppInterstitials into one 98-effective-line owner capturing log and importing existing app/input helpers. Complete function bodies are preserved apart from factory indentation; inverse reconstruction proves the remaining root and all call sites unchanged. Entry falls from 5,966 to 5,874 effective lines. Added 24 direct contracts, including two real offline Chromium cases, plus the explicit package path.

Paired full baseline/final Node suites pass 350/350 with identical identities and warnings and no skips. Paired nested-package contract passes 1/1. Checker tests pass 19/19; source proof, UTF-8/no-BOM, syntax, ratchet and both Git checks pass. Strict remains an expected exit 1: 1,917 scanned, 16 hard, 24 mandatory, 40 soft.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 5874 effective lines.
- scripts/gemini-canvas-browser-pool-conversation-reset.mjs: 98 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 116 effective lines.
- scripts/tests/gemini-canvas-browser-pool.conversation-reset.test.mjs: 160 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 184 effective lines.

Evidence: target/effective-line-evidence/20260914-browser-pool-conversation-reset-owner/scope.json.
SHA-256: d02f7ede39ba87f0584d2deb2eb6c22ffa31b0499e0581f02b93434d8f771433.
Predecessor: 33636d1db8bc8cc308eaabd23abed43e77d950ba077dae7a36f0d49ea917cdf9.
Union 761; unchanged neighbors 758; web assets preserved.
Strict: 1917 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Continue a bounded program-handle/capture ownership extraction after checking exact dependencies; retain URL-prefix admission and pre-try media reset cleanup as separate behavior risks. S06 and whole-plan release gates remain open.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
