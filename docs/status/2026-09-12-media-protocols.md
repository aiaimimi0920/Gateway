# LumaLabs, Suno and Udio protocol extraction

Accepted on 2026-09-12. This is a structural checkpoint in the continuing Gateway
plan, not a completed plan or a release acceptance.

## Ownership and preservation

The LumaLabs, Suno and Udio protocol entries decreased from 1,487/1,175/1,360
to 119/119/114 effective lines. The 35 scoped source/test files are all at most
288 effective lines. Request options, generation/actions, result decoding,
output envelopes and worker contracts have explicit provider-local owners.
LumaLabs runtime material and event URL recovery are separate owners; Suno/Udio
scalar decoding remains private to each provider family. Roots retain their
schemas, operation definitions, constants and canonical entry behavior.

Complete item comparisons preserve 152 moved and eight retained production
items, their adjacent inherent implementations, 27 constants, every public item
path, 75 original tests and four fixtures. The provider leaf dependencies are
acyclic. All 15 files from the accepted protocol-family checkpoint are unchanged.
No production behavior hardening is included in this extraction.

The initial all-targets compile exposed child-module resolution under explicit
entry `#[path]` attributes. The fix removes exactly the three redundant enabled
attributes in `src/protocol/mod.rs`, matching the existing Kiro declaration.
Every feature condition, disabled branch and other declaration is preserved.
The failed diagnostic remains in `initial-all-targets.log`; the corrected gate
has its own fresh log.

## Verification

- Paired default-feature LumaLabs library tests: 29/29 before and after.
- Paired Suno library tests: 22/22 before and after.
- Paired Udio library tests: 24/24 before and after.
- Fresh offline, locked all-targets compilation: passed.
- Scoped provider formatter and module-only formatter: passed.
- Effective-line checker tests: 19/19; ratchet: passed.
- Exact item, constant, public-path and module-declaration comparisons: passed.
- UTF-8 without BOM, final newlines, scoped whitespace and both Git diff checks:
  passed.

Global formatting still reports only the S06-owned
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`. Strict inventory remains
nonzero: 1,454 files scanned, 31 hard, 47 mandatory and 40 soft. There are now
78 files above 700, down from 81. Structural clearance is 67/145 (46.2%). Neither
the baseline nor the exception policy was changed.

Immutable evidence:
`target/effective-line-evidence/20260912-media-protocols/scope.json`.
The same directory contains original source snapshots, extraction verifiers,
paired test logs, both compile diagnostics, formatting logs, checker/ratchet/
strict results and independent Git checks. The scope capture was taken at
2026-09-11 20:19:01 UTC, before this documentation closure.

At that capture, Gateway HEAD was
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`, with 1,566 status entries:
159 modified, one unstaged deletion, two staged deletions and 1,404 untracked.
Neuro HEAD was `bf818f0324024634bc890585efb78cc8e603d11a`, with 192 entries:
10 modified and 182 untracked. These include inherited work and are not an
attribution of all changes to this batch.

## Remaining work

At this structural checkpoint, worker diagnostics still interpolated raw details
and local paths. The subsequent [diagnostic hardening](2026-09-12-media-worker-errors.md)
closes that constructor boundary with separate failing-then-passing evidence.
Upstream failure-message overrides and original process-output allocation remain
separate boundaries. The structural snapshots and test evidence above are immutable.

No browser, real provider, production runtime or packaged release was exercised.
S06 retains its implementation and original cursor. GWP-20260908-06 still needs
an explicit source/docs freeze and shared release-build transfer receipt. All
coordinator native gates for this checkpoint are terminal; no new release was
built.
