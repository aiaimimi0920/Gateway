# Rate-limit-hotspot ownership

Owner: parallel coordinator. State: verified. Started: 2026-09-13 UTC.

Accepted: 2026-09-13 03:35:59 UTC. The entry is 337 effective lines; all ten
source/test owners are at most 397. Exact proof preserves 68 production
definitions, 32 public paths, seven crate-visible builder paths, 21 concrete DTOs,
five tests, three fixtures and 411 neighbors. Only 18 helpers gain pub(super).
Paired hotspot/incident/remediation tests pass 5/5, 6/6 and 21/21 with identical
test identities and warning sets. Fresh all-targets, scoped formatting, checker
19/19, ratchet, source/encoding/cleanup proof and both Git checks pass. Strict:
1,668 scanned, 25 hard, 27 mandatory and 40 soft; 52 remain above 700, clearance
93/145 (64.1%). The pre-apply transport-guard correction is preserved in evidence.
Scope SHA-256: a3b29ee08425412ff7125d470cf3e55ede650a2e1341847d82e814e1b851ccc4.
[Acceptance report](../../status/2026-09-13-rate-limit-hotspot-owners.md).

Scope: src/db/rate_limit_hotspots.rs and private rate_limit_hotspots/ owners.
The complete 1,812-effective-line entry, public database reexports, management
HTTP callers and existing hotspot/remediation tests were inspected. Retain the
21 concrete DTOs at the entry. Separate audit queries, aggregation, anomaly
thresholds/reports, normal/anomaly snapshot operations, snapshot aggregation,
bucket metrics and shared snapshot storage/filtering. Keep the five existing
unit tests and three fixtures together; every owner must remain below 500.

Preserve all 47 function bodies, 32 public paths, seven crate-visible builder
paths, serde/defaults, query limits, rate-limit classification, bucket ordering,
time-window boundaries, concentration/threshold arithmetic, profile precedence,
snapshot keys, object-store operations, malformed-object skipping and diagnostic
text. Widen only the private helpers required by sibling owners to pub(super).
No schema, arithmetic, persistence, error-handling or resource-lifecycle change
belongs to this structural batch.

Paired gates cover the five hotspot unit tests and the existing anomaly-incident
and remediation test modules. Require fresh locked offline all-targets, scoped
formatting, checker 19/19, ratchet/strict inventories, exact definition/public/
crate-path/type/test/fixture proof, UTF-8/no-BOM checks, terminal Cargo cleanup and
both Git diff checks. Pure unit gates do not claim live PostgreSQL/object-store
or packaged runtime validation.

Evidence: target/effective-line-evidence/20260913-rate-limit-hotspot-owners/.
Serialize Cargo and do not edit Rust while a gate runs. Preserve the accepted
database-routing snapshot and all inherited neighboring inputs. S06 keeps its
implementation/cursor; GWP-20260912-01, runtime-profile governance and final
freeze/build transfer remain pending. No dependency, baseline, exception,
checker-policy, release or persistent deployment change belongs here.
