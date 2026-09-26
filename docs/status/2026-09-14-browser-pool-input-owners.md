# Browser-pool input ownership checkpoint

Accepted at 2026-09-14 00:15:37.332 UTC. This accepts the first incremental S18
boundary, not the remaining browser-pool entry or the whole release plan.

The entry decreases from 10,462 to 10,424 effective lines. Four intact functions
move to scripts/gemini-canvas-browser-pool-input.mjs (44 effective lines). Root
imports preserve all call sites; exact projection preserves every root byte
outside the removed block and import wiring. normalizeString has one definition.
normalizeObject keeps its separate object/array rule. No backwards dependency,
environment read, I/O, module state or startup side effect enters the pure owner.

The fixture gains two private test exports (52 -> 54 lines). Six new contracts
(74 lines) cover coercion, object identity, numeric account slots, caller state,
all four URL fields, share query/fragment handling, foreign hosts, malformed URLs,
arrays and empty URL semantics. These tests are frozen before production edits.
All 40 browser-pool Node tests pass before and after, with identical identities,
zero skips/cancellations and no warnings. Temporary import fixtures leave no leak.

The nested package contract grows 143 -> 155 lines. Its original assertions
remain byte-identical after removing the three added support paths and new probe.
Both phases pass the same Python test. Final coverage verifies root/resources/input
module bytes, manifest records, lengths and checksums, then imports the packaged
pure module outside the source tree. The fixture uses synthetic binaries. It
does not establish a production release, browser startup or Docker execution.
Dockerfile and packager remain unchanged and already copy ordinary scripts recursively.

Fresh source/UTF-8-no-BOM/syntax checks, checker 19/19, ratchet and separate
Gateway/Neuro diff checks pass. No Node formatter is configured; original source
formatting and hygiene are preserved. Gates are terminal. No Cargo, provider,
browser, live service, dependency, policy, baseline or exception changes occurred.
The 693-input union preserves 690 frozen neighbors and published web assets.
Independent read-only review found no concrete regression.

Strict: 1,850 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance
stays 105/145 (72.4%). Residual browser-pool entry debt remains open. No new global
Rust formatting gate was needed; the preceding send acceptance recorded the two
reserved S06 formatting failures, and those sources are frozen unchanged here.

Immutable evidence: target/effective-line-evidence/20260914-browser-pool-input-owners/scope.json.
SHA-256: 735aa41474724457612dfdfbb1d0bad4b2d20f8e43e5508153dc53e8d484ee30.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,069 entries
(182 modified, one unstaged deletion, two staged deletions, 1,884 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: browser executable selection and TLS configuration ownership, with fresh
pre-extraction contracts. Profile clone/context lifecycles remain in the entry
until their separate ownership boundary is designed. S06 implementation/cursor
and final build coordination remain reserved under GWP-20260912-01. Full strict,
language/provider/release gates, runtime-profile governance, immutable packaging
and packaged runtime/UI/Docker validation remain open. Final runtime target stays
persistent 4200 and no persistent 4226; no deployment occurred.
