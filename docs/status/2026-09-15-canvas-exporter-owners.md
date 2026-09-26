# Canvas exporter auth and runtime capture owners checkpoint

Accepted at 2026-09-14T23:44:51.701Z. Entry 761 -> 495 effective lines.

Authentication/page signals move to a 184-line owner; runtime material discovery
and collection move to a 116-line owner. Complete blocks, private constants,
localized text and browser callbacks remain intact. Both owners reuse the existing
pure normalizer with a byte-identical function body. The root keeps its public
exports, CLI guard, manual-login polling, object-storage client, output schemas
and browser/context cleanup. Exact inverse projection reconstructs the original root.

Paired focused exporter Node suites pass 25/25, retaining nine existing identities
and adding sixteen runtime contracts, with identical warnings and zero skips.
Tests cover native public imports and actual callback serialization, auth signals,
page identity/order, preferred-page selection, cross-source key deduplication and
transient/nontransient error paths. All material is synthetic; no live login occurs.
Paired package 1/1 adds the exporter and two owners to original byte/manifest/checksum
assertions. Independent reviews found no introduced defect. The authoritative
baseline was rerun serially after both reviewers ended; early receipts remain
archived and are not used for acceptance. Accepted gate intervals do not overlap.
The preceding browser-pool 932-test evidence is preserved by hashes and was not
rerun for this isolated exporter change.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/export-gemini-canvas-storage-state.mjs: 495 effective lines.
- scripts/export-gemini-canvas-auth-signal.mjs: 184 effective lines.
- scripts/export-gemini-canvas-runtime-capture.mjs: 116 effective lines.
- scripts/tests/export-gemini-canvas-storage-state.fixtures.mjs: 48 effective lines.
- scripts/tests/export-gemini-canvas-storage-state.runtime.test.mjs: 97 effective lines.
- scripts/tests/export-gemini-canvas-storage-state.test.mjs: 220 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 213 effective lines.

Evidence: target/effective-line-evidence/20260915-canvas-exporter-owners/scope.json.
SHA-256: ad08412dfda97df38c038d44af79944505b370e2361f6fefc1bfc5dd81f9e354.
Predecessor: 3a5f0e3483faa88cca9be43b27e0e41110ff65c45300ec964735f00fdec8292a.
Union 838; unchanged neighbors 836; web assets preserved.
Strict: 1991 scanned, 15 hard, 23 mandatory, 40 soft;
38 above 700. Clearance 107/145 (73.8%).

Next: Inspect the 2283-line browserless program probe and its HTTP/auth request boundaries.
Establish focused preservation contracts before extracting cohesive owners <=500.
No probe source or owner is changed in this checkpoint.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
