# LumaLabs, Suno and Udio upstream response ownership

Owner: parallel coordinator. State: `structural_green`.
Started: 2026-09-12.

Scope is `src/upstream/lumalabs_response_helpers.rs`,
`src/upstream/suno_response_helpers.rs`, `src/upstream/udio_response_helpers.rs`
and their new private production/test directories. Original effective counts are
717/1,123/1,059. All three complete sources and their tests have been read directly;
byte-equal source snapshots are retained before any production extraction.

Separate browser worker result admission, HTTP/challenge classification, HTTP JSON
decoding and media output materialization. Retain typed execution-plan ownership
and meaningful response orchestration at the existing entry paths. Keep all
crate-visible signatures, ordering, timeout calculations, status/code precedence,
output readiness, requested counts and MIME behavior unchanged. Private children
use direct imports in one direction; do not add cross-provider utility layers.

The fresh caller gates from the accepted diagnostic checkpoint provide the
pre-extraction baseline: LumaLabs 30/30, Suno 52/52 and Udio 40/40. The copied logs
are preserved as the paired baselines; they ran against these unchanged source
snapshots. All 122 tests and six fixtures must remain unchanged and be split by
responsibility. Every resulting source/test owner must be below 500 effective
lines. Module declarations already use ordinary basename resolution.

Evidence: `target/effective-line-evidence/20260912-media-responses/`. After moving
complete items, verify full body/signature/test comparisons, acyclic child imports,
unchanged neighboring protocol and caller sources, all-targets compilation, the
same tests, the accepted seven diagnostic regressions, scoped formatter,
checker/ratchet/strict, encoding and independent Git states.

The source extraction and three full body comparisons now pass: 53 moved and
15 retained production items, all 122 tests and six fixtures. Entries measure
149/82/127 effective lines; all 26 scoped source/test files are at most 247.
The first all-targets and paired gates passed, but explicit re-exports introduced
unused-import warnings for helpers now called inside their owners. All 68 existing
crate-visible item paths are retained, with a documented lint allowance limited
to the nine re-export declarations. No implementation or test body changes.
Fresh final gates now pass on this completed namespace layout: all-targets,
LumaLabs 30/30, Suno 52/52, Udio 40/40 and worker diagnostics 7/7. Scoped formatting,
checker 19/19, ratchet, encoding and both Git checks pass. Global formatting reports
only the two unchanged S06 runtime-mirror files. The 41 neighboring files and all
68 crate-visible paths are preserved. Immutable `scope.json` was captured at
2026-09-11 22:06:46 UTC. Strict scans 1,478 files, with 31 hard, 44 mandatory and
40 soft; 75 remain above 700. Clearance is 70/145 (48.3%).

Acceptance: [media response report](../../status/2026-09-12-media-responses.md).
All coordinator native gates are terminal. Do not regenerate this extraction
evidence after a deliberate hardening change.

Upstream failure-message overrides, Suno's JSON error preview, original body/output
allocations and image copies remain explicit hardening work after the structural
checkpoint. No behavior fix is folded into this extraction. S06 retains its
implementation and original cursor. GWP-20260908-06 source/docs freeze and shared
release-build transfer remain unreceipted; scoped Cargo gates do not authorize a
release build.
