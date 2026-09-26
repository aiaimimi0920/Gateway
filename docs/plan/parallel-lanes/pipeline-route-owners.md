# Pipeline route-stage ownership

Owner: resumed parallel coordinator. State: accepted at 2026-09-13 19:32:09.384 UTC.

Entry 1,027 -> 403 effective lines; six owners, maximum 403. Paired gates pass
8/16/10/1 with identical tests/warnings. All-targets, scoped formatter, checker
19/19, ratchet, source/encoding/diff proof and fixture cleanup pass. Accepted union
632; 626 neighbors preserved. Strict: 1,812 scanned, 18 hard, 24 mandatory,
40 soft; 42 above 700, clearance 103/145 (71.0%).
Evidence: target/effective-line-evidence/20260914-pipeline-route-owners/scope.json.
SHA-256: 53b618772ebb8a4daecefca191c610f6747d3d5f26af38fb085dd336c1963561.
See [acceptance report](../../status/2026-09-14-pipeline-route-owners.md).

Exclusive source scope: src/pipeline/stage_route.rs and new private owners under
src/pipeline/stage_route/. The original entry is 1,027 effective / 1,164 physical
lines. The coordinator read the entire source, eight tests and six fixtures.
This independent S15 boundary does not transfer S06 or final shared-build control.

Retain the complete public run body at the parent. Extract only the five existing
private account-group and candidate-policy functions. Split tests into resolution
and account-group contracts, retaining fixture/test bodies and raw YAML bytes.
Every result must be at most 500 effective lines. Preserve credential ownership,
Redis/PostgreSQL/YAML precedence, access balance pre-deduction before finalization,
candidate/projected-row alignment, group filtering before affinity and queue,
health collection before queue construction, and execution-mode assignment after
ordering. No streaming-body, resource-lifecycle or runtime-policy change is planned.

Pin routing-config acceptance and neighboring inputs before source changes.
Paired gates cover the eight route-stage tests, sixteen protocol candidate-selection
tests, ten queue tests and bounded health-collection test. These tests do not
exercise the database-backed access-key path end to end; exact run-body preservation
also protects its ordering. Redis attempts use localhost:6379; fixtures use no
PostgreSQL pool. Record actual runtime results and temporary fixture cleanup.

Serialize all Cargo/compiler/formatter operations with idle guards. Keep
GATEWAY_PREBUILT_WEB_UI=1 per process and preserve published web assets. Final
all-targets, scoped formatter, checker tests, ratchet, strict inventory, source
and encoding proof, and separate repository diff checks remain required. Preserve
all inherited changes; no dependency, policy, baseline, exception, live service,
release or S06-owned source change is authorized by this lane.
