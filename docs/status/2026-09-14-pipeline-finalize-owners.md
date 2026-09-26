# Pipeline finalization ownership acceptance

Accepted at 2026-09-13 20:34:28.427 UTC (2026-09-14 local time). The whole
Gateway optimization and release plan remains open.

The finalization entry decreases from 1,820 to 128 effective lines. All twelve
source/test owners are at most 352:

| File under src/pipeline | Effective lines | Responsibility |
| --- | ---: | --- |
| stage_finalize.rs | 128 | Public DTOs, public paths and private module wiring |
| stage_finalize/buffered.rs | 148 | Buffered success finalization |
| stage_finalize/audit_begin.rs | 116 | Audit creation and snapshots |
| stage_finalize/failure.rs | 94 | Failure refund and finalization |
| stage_finalize/streaming.rs | 82 | Streaming success finalization |
| stage_finalize/archive.rs | 224 | Conversation archive persistence |
| stage_finalize/audit_persistence.rs | 187 | Audit and usage persistence |
| stage_finalize/credential_model.rs | 155 | Credential-model persistence |
| stage_finalize/route_trace.rs | 352 | Route trace projection |
| stage_finalize/quota.rs | 188 | Quota refund and settlement |
| stage_finalize/timestamps.rs | 41 | Timestamp conversion |
| stage_finalize/tests.rs | 155 | Ten unchanged tests and one fixture |

Exact proof preserves 43 production definitions, nine public root paths, both
DTOs and their fields, ten tests/one fixture, function bodies and side-effect
ordering. Cross-owner private helpers gain only pub(super); three test-only
imports are cfg-gated. All 633 neighboring inputs and published web assets are
unchanged. Independent read-only projection review found no concrete regression.

Failure still refunds before audit/archive/model work and failure usage reporting.
Buffered success retains usage, audit, archive, model, quota and affinity order.
Streaming success retains completion semantics, usage, audit, archive, model and
quota order. The existing TrackedStream callback owns terminal dispatch; these
finalization functions gain no separate idempotency guarantee. Existing guards,
bounded archive delegation and callback captures are unchanged.

Paired finalization, upstream stream and archive contracts pass 10/10, 26/26 and
6/6 with identical test identities and warning sets. They use in-memory requests
and streams or pure archive contracts; no new external database/Redis fixture is
introduced. These tests do not prove database quota transactions or completion of
detached tasks during runtime shutdown.

Fresh locked offline all-targets, scoped rustfmt, source/encoding proof, checker
19/19, ratchet and separate Gateway/Neuro diff checks pass. Compiler/formatter
gates were serialized with idle guards and per-process GATEWAY_PREBUILT_WEB_UI=1.
All owned native handles are terminal. No external process, dependency, checker
policy, baseline or exception was changed.

Strict scans 1,823 files: 17 hard, 24 mandatory, 40 soft; 41 remain above 700.
Clearance is 104/145 (71.7%). Strict exits 1. Global formatting still exits 1 only
for the two reserved gemini_canvas_runtime_mirror source/test files.

Immutable evidence: target/effective-line-evidence/20260914-pipeline-finalize-owners/scope.json.
SHA-256: 974ee55889a10a00db970bf7bb51f3dc7c86788d0adb3049b8fe38031792bb66.
Acceptance validates the 645-input union, prior/current/saved hashes, exact
source/projection and tests/warnings, serialized timing, web assets and Git.

Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,037 entries
(182 modified, one unstaged deletion, two staged deletions, 1,852 untracked).
Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). This census precedes documentation publication.

Stage-send ownership is the next S15 boundary. Its large run function needs
control-flow extraction and populated-candidate HTTP/SSE regression proof first.
S06 implementation/cursor and final build transfer remain reserved under
GWP-20260912-01. Residual strict debt, runtime-profile governance, full language/
provider/release gates, immutable packaging and packaged runtime/UI/Docker
validation remain open. No release or persistent deployment occurred; the final
runtime target remains persistent 4200 and no persistent 4226.
