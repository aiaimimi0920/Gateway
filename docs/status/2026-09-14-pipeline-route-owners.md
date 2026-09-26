# Pipeline route-stage ownership acceptance

Accepted at 2026-09-13 19:32:09.384 UTC (2026-09-14 local time). The whole
Gateway optimization and release plan remains open.

The route-stage entry decreases from 1,027 to 403 effective lines. All six
source/test files are at most 403:

| File | Effective lines | Responsibility |
| --- | ---: | --- |
| src/pipeline/stage_route.rs | 403 | Unchanged public run and private module wiring |
| src/pipeline/stage_route/account_group.rs | 46 | Account-group errors and paired candidate filtering |
| src/pipeline/stage_route/candidate_policy.rs | 63 | Protocol finalization and family-policy admission |
| src/pipeline/stage_route/tests.rs | 156 | Six unchanged fixtures and test module wiring |
| src/pipeline/stage_route/tests/resolution.rs | 114 | Three route-resolution contracts |
| src/pipeline/stage_route/tests/account_groups.rs | 257 | Five exclusion and alignment contracts |

Exact proof preserves all six production functions, eight tests, six fixtures,
seven raw YAML literals and the public run path. Only five private helpers gain
pub(super). Run is byte-for-byte identical after line-ending normalization,
including access pre-deduction, credential isolation, fallback precedence,
candidate/projected-row alignment, group-before-affinity/queue ordering, bounded
health collection and execution-mode assignment. All 626 neighboring inputs and
published web assets are unchanged.

The independent reviewer found no helper extraction regression. Its two claims
about a changed database fallback and health collector were disproved by direct
inspection of the pinned original and current run item. The source and complete
run hashes, exact locations and disposition are recorded in review.md; no
speculative behavior change was added.

Paired route-stage, protocol-candidate, queue and health contracts pass 8/8,
16/16, 10/10 and 1/1 with identical warning sets and exact test identities after
the recorded nested-module mapping. Fixtures use no PostgreSQL pool; localhost
Redis attempts remain unchanged. These gates do not establish database-backed
access-key E2E coverage.

Each phase created seven UUID console-state directories. The harness verified
containment, symlink absence and bounded tree size, removed only those new
fixtures after Cargo terminated, and preserved all 136 inherited directories.
Fixed-name YAML fixtures were absent before tests and none remains afterward.

Fresh locked offline all-targets, scoped rustfmt, source/encoding proof, checker
19/19, ratchet and separate Gateway/Neuro diff checks pass. Every compiler and
formatter gate was serialized with idle guards and per-process
GATEWAY_PREBUILT_WEB_UI=1. Owned native handles are terminal; no external process
was terminated. No dependency, checker policy, baseline or exception changed.

Strict scans 1,812 files: 18 hard, 24 mandatory, 40 soft; 42 remain above 700.
Clearance is 103/145 (71.0%). Strict exits 1. Global formatting still exits 1
only for the two reserved gemini_canvas_runtime_mirror source/test files.

Immutable evidence: target/effective-line-evidence/20260914-pipeline-route-owners/scope.json.
SHA-256: 53b618772ebb8a4daecefca191c610f6747d3d5f26af38fb085dd336c1963561.
Acceptance validates the 632-input union, prior/current/saved hashes, exact
source/projection and tests/warnings, serialized timing, fixture cleanup and Git.

Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,024 entries
(182 modified, one unstaged deletion, two staged deletions, 1,839 untracked).
Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). This census precedes documentation publication.

Pipeline finalization ownership is the next independent S15 boundary. S06
implementation/cursor and final build transfer remain reserved under
GWP-20260912-01. Residual strict debt, runtime-profile governance, full language/
provider/release gates, immutable packaging and packaged runtime/UI/Docker
validation remain open. No release or persistent deployment occurred; the final
runtime target remains persistent 4200 and no persistent 4226.
