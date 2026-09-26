# Browser pool music late-event hardening checkpoint

Accepted at 2026-09-14T09:15:54.839Z. Entry 5966 -> 5966 effective lines.

Both closure-free music executors now reject queued open/message callbacks after settlement and recheck settlement after asynchronous text decoding. The three guards per owner prevent late setup sends, unnecessary decodes, and idle-timer recreation. Root wiring is unchanged; each owner grows from 302 to 311 effective lines. Rejected in-flight text decoding, malformed base64, unbounded audio storage, permissive admission, and the pre-try media reset cleanup gap remain outside this batch.

Focused regressions reproduced 0/8 passing before the guards and pass 8/8 afterward. Full final Node tests pass 326/326 with no skips; nested package contract passes 1/1. Checker tests pass 19/19. Source proof, UTF-8/no-BOM, syntax, ratchet, and both repository diff checks pass. Strict remains an expected exit 1: 1,915 scanned, 16 hard, 24 mandatory, 40 soft. This batch has focused red/green evidence and a full final run; it does not claim paired full-suite baseline/final runs.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 5966 effective lines.
- scripts/gemini-canvas-browser-pool-preview-music.mjs: 311 effective lines.
- scripts/gemini-canvas-browser-pool-page-music.mjs: 311 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 113 effective lines.
- scripts/tests/gemini-canvas-browser-pool.no-key-music.test.mjs: 246 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 183 effective lines.

Evidence: target/effective-line-evidence/20260914-browser-pool-music-late-event-hardening/scope.json.
SHA-256: 33636d1db8bc8cc308eaabd23abed43e77d950ba077dae7a36f0d49ea917cdf9.
Predecessor: 71fa34608025465195f3287f2814e9e49ee53b4e715d593540063adac7f033ba.
Union 759; unchanged neighbors 757; web assets preserved.
Strict: 1915 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Extract conversation-reset ownership with behavior-preserving contracts; retain S06 reservation and all whole-plan language, provider, release, runtime, UI, and Docker gates.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
