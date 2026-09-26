# Browser pool text execution owner checkpoint

Accepted at 2026-09-14T18:45:58.287Z. Entry 3409 -> 3135 effective lines.

Ten complete functions moved from two text-only blocks into a 293-effective-line
owner; bodies differ only by factory indentation. Interleaved media predicates and
root call sites are preserved. Two direct imports and seven injected dependencies
retain initialization order, serialized snapshots, exact-answer augmentation, baseline
comparison, prompt anchoring, stable/deadline selection and capture-before-reset cleanup.

Paired full Node suites pass 627/627 with identical identities/warnings and no skips.
Paired package contract passes 1/1 with only the owner path added. Thirty-three new
contracts cover actual-root prompt/snapshot helpers and isolated real-body lifecycle
execution. The latter does not alone prove injection; source projection and actual-root
imports/probes provide separate wiring checks. Independent projected review found no
introduced dependency, lifecycle or schema defect.
Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged-runtime claim.

- scripts/gemini-canvas-browser-pool.mjs: 3135 effective lines.
- scripts/gemini-canvas-browser-pool-text.mjs: 293 effective lines.
- scripts/tests/gemini-canvas-browser-pool.fixtures.mjs: 176 effective lines.
- scripts/tests/gemini-canvas-browser-pool.text.test.mjs: 137 effective lines.
- scripts/tests/gemini-canvas-browser-pool.text-fixtures.mjs: 69 effective lines.
- tests/python/test_gateway_nested_worker_package_contract.py: 197 effective lines.

Evidence: target/effective-line-evidence/20260915-browser-pool-text-owner/scope.json.
SHA-256: 0640eca93b0ab06211f8ebdb6ebf62f713d302138f05c78781ed362553c9a10f.
Predecessor: 643c54297a6d56597efd3442434e54ee129ba27136e11b038ca9ea41b28cfee2.
Union 796; unchanged neighbors 793; web assets preserved.
Strict: 1952 scanned, 16 hard, 24 mandatory, 40 soft;
40 above 700. Clearance 105/145 (72.4%).

Next: Prove and fix the existing runDebugOperation failure-path capture leak in a
separate behavioral batch before moving debug orchestration. Current stop is only on
the success path. Page/button snapshot bodies have also been read in full. Bootstrap
phase extraction still needs its full source read and one adoption/finally boundary.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
