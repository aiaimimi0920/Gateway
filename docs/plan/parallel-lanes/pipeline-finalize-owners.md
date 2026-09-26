# Pipeline finalization ownership

Owner: resumed parallel coordinator. State: accepted at 2026-09-13 20:34:28.427 UTC.

Before snapshot pins 634 inputs and pipeline-route acceptance. Original SHA-256:
d4d85ec660d112032c08b09ecc0f9854b6e27405406cc99b18b8c1fdae97b336.
Projection: twelve files, entry 128, maximum 352 effective lines. Exact proof
preserves 43 production definitions, nine public paths, ten tests/one fixture and
both snapshot DTOs. Independent read-only projection review found no concrete
regression. Baseline finalization/stream/archive gates passed 10/26/6, all native
handles terminated, and guarded application/candidate proof passed. All 633
neighbors remain exact. Final gates pass 10/26/6 with identical identities and
warnings; all-targets and scoped closing gates pass. Accepted union: 645.
Strict: 1,823 scanned, 17 hard, 24 mandatory, 40 soft; 41 above 700, clearance
104/145 (71.7%). Global formatting retains only the two reserved S06 paths.

Evidence: target/effective-line-evidence/20260914-pipeline-finalize-owners/scope.json.
SHA-256: 974ee55889a10a00db970bf7bb51f3dc7c86788d0adb3049b8fe38031792bb66.
[Acceptance report](../../status/2026-09-14-pipeline-finalize-owners.md).

Exclusive scope: src/pipeline/stage_finalize.rs and new private children in
src/pipeline/stage_finalize/. Original inventory: 1,820 effective / 1,977 physical
lines. The coordinator read the full source, ten tests/one fixture, direct stream
callback and archive contracts. S15 requires a stable finalization boundary before
stage_send changes; this lane does not transfer S06 or final build coordination.

Keep both shared public snapshot DTOs at the parent. Extract unchanged functions
into buffered success, audit creation/snapshots, failure, streaming success,
archive persistence, audit persistence, credential-model persistence, route trace,
quota and timestamp owners. Preserve all public root paths and DTO fields. Every
file must be at most 500 effective lines; test paths/bodies remain unchanged.

Preserve refund-before-failure-audit ordering; usage/audit/archive/credential-model/
quota/affinity success ordering; existing no-session and no-database guards;
existing TrackedStream callback terminal dispatch; pre-deduction conditions;
bounded archive input and redaction delegation. No new streaming collection,
clone, task, retry or database/Redis operation is planned. Do not change callers.

Pin pipeline-route acceptance and all neighboring inputs. Paired tests cover ten
finalization tests, the upstream stream suite and archive sanitizer/endpoint/
classification contracts. These tests do not prove external quota transactions or
runtime-shutdown completion of detached work; preserve those bodies and callers
exactly and retain that limit in the acceptance report.

Serialize Cargo/compiler/formatter operations with idle guards and per-process
GATEWAY_PREBUILT_WEB_UI=1. Final all-targets, scoped formatting, checker tests,
ratchet, strict inventory, source/encoding proof and separate Git diff checks are
required. No policy/baseline/exception, dependency, live service or release edits.
