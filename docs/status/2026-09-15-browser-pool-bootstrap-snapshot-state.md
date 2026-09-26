# Browser pool Bootstrap snapshot state checkpoint

Accepted at 2026-09-14T19:56:06.989Z. Entry 2915 -> 2768 effective lines.

Eleven identical 16-line snapshot merge sequences now use one synchronous local
closure. Handle/action/invoke order, exact snapshot identity, current capture lookup,
page adoption and the outer finally remain. Preview and normal fallback assembly
remain separate, including the adopted preview snapshot that bypasses normal merges.
Bootstrap decreases from 716 to 569 effective lines; further phase extraction is required.
No new production owner or package path is introduced.

Paired full Node 678/678 with identical identities/warnings and zero skips; paired
nested package 1/1, with the package contract byte-identical. Twenty-three new
contracts cover merge order, late fields, popup cleanup, proxy aliasing, early
results/errors, media fallbacks and deadline/stabilization behavior. They use the
real operation body and capture owner with controlled pages/dependencies/time.
Exact inverse reconstruction restores every original block and outside bytes.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 2768 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 183 effective lines.
- scripts/tests/gemini-canvas-browser-pool.bootstrap.test.mjs: 187 effective lines.
- scripts/tests/gemini-canvas-browser-pool.bootstrap-fixtures.mjs: 116 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-bootstrap-snapshot-state/scope.json.
SHA-256: a2b4b7a36c5b57561e15e03705e1aa25c82e06842d848716532ecdac8ab4ede8.
Predecessor: d6e4e6817368e7538ca0658f9a7bd6d1064961e64cfe732b5d1c53c5041f34a4.
Union 803; unchanged neighbors 802; web assets preserved.
Strict: 1959 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Extract the preview result-construction phase (root 636-730, 95 effective lines)
with explicit state/snapshot inputs and preserved return/throw semantics. Probe and
navigation phases have greater page/capture coupling. Do not move the whole
569-line Bootstrap function into a new owner; each new owner must stay <=500.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
