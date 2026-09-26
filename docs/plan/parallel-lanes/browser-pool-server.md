# Browser pool server lifecycle and media policy owners ownership

Owner: resumed coordinator.

Accepted at 2026-09-14T23:24:34.988Z. Entry 808 -> 484 effective lines.

The complete server lifecycle moves to a 157-line owner with seven same-source
imports and twelve initialized dependencies. HTTP/HTTPS routing, request parsing,
WebSocket ownership, idle eviction and the suppression/fatal guard remain intact.
A separate 221-line pure media-policy owner preserves progress, provider-gate and
resubmission decisions, localized matches and its private RPC allowlist. Six root
imports retain native helper bindings. Exact inverse projection restores both
complete blocks and the original root; the browser retry executor stays unchanged.

Paired full Node 932/932 retains all 870 preceding identities and adds 44 server
and 18 media-policy contracts, with identical warnings and zero skips. Baseline
runs the original 808-line root; candidate runs bound/imported owners. Controlled
server tests protect routes, identities, errors, startup order, TLS and socket
callbacks; native-root policy tests protect precedence, history bounds and input
immutability. Independent reviews found no introduced blocker. Paired package 1/1
protects both owner paths with the existing byte/manifest/checksum contract.

The first server-only candidate reached 689 and passed paired 914/914 plus package
1/1, but ratchet required a current 501-700 exception. That failed evidence remains
immutable. Original accepted root/package bytes were restored for a fresh expanded
baseline; adding the pure policy owner brings root to 484 without changing any
exception, policy or baseline. Nine failed-attempt receipt hashes are preserved.
These are controlled contracts, not real provider or packaged-runtime validation.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 484 effective lines.
- scripts/gemini-canvas-browser-pool-server.mjs: 157 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 192 effective lines.
- scripts/tests/gemini-canvas-browser-pool.server-fixtures.mjs: 88 effective lines.
- scripts/tests/gemini-canvas-browser-pool.server-http.test.mjs: 119 effective lines.
- scripts/tests/gemini-canvas-browser-pool.server.test.mjs: 112 effective lines.
- scripts/gemini-canvas-browser-pool-media-policy.mjs: 221 effective lines.
- scripts/tests/gemini-canvas-browser-pool.media-policy.test.mjs: 55 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 210 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-server-v2/scope.json.
SHA-256: 3a5f0e3483faa88cca9be43b27e0e41110ff65c45300ec964735f00fdec8292a.
Predecessor: 5efcb4b691c5b28b8eecec1f9e25e4020dd32d7f6674f67c31bed4a861016f26.
Union 831; unchanged neighbors 829; web assets preserved.
Strict: 1987 scanned, 15 hard, 24 mandatory, 40 soft;
39 above 700. Clearance 106/145 (73.1%).

Next: Inspect the 761-line storage-state exporter and existing authentication, candidate
page and API-key tests. Plan cohesive auth/runtime boundaries so both the resulting
root and new owners are <=500; no exporter source or owner is changed here.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.

[Checkpoint report](../../status/2026-09-15-browser-pool-server.md).
