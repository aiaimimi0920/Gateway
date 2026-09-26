# Remediation ownership

Owner: parallel coordinator. State: verified. Accepted: 2026-09-13 07:26:01 UTC.

Scope: src/db/remediation.rs and private remediation/ owners. The complete
6,932-effective-line source, database reexports, management handlers, export
consumer and desktop contracts/schemas were inspected. The 48 public DTOs total
685 effective lines and moved into action, policy, run, effectiveness and
effectiveness-anomaly owners with unchanged public entry reexports. Shared
private records/configuration stay at the entry; private fields stay private.

Separate plan construction/context, alerts, queue scheduling, sweep execution,
durable runs, execution/history, route-policy patches, impact, effectiveness,
snapshot/trend/anomaly reports, policy reads/writes/parameters/sync/sweep and
auto-remediation configuration. Keep the 460-line plan builder cohesive within
its own owner. Preserve all 186 production items, 73 public paths, 21 original
tests and four fixtures. Split tests by action, scheduling, sync and effectiveness.

SQL/binds, side-effect order, errors, serialization, filtering, arithmetic,
dry-run behavior, rate-limit tightening, schedules, object storage operations,
provider-health behavior and resource lifetime remain unchanged. Execution
failure recording, policy_id binding and extreme timestamp arithmetic are
separate suspected defects; this extraction must preserve their existing behavior.

Evidence: target/effective-line-evidence/20260913-remediation-owners/.
Original raw/canonical SHA-256:
e06b8589f3a40b15da7a4d33cb9f775d179e69c9638a18d8c72caf90c4e2320d.
Predecessor: the accepted analysis-export text-policy checkpoint, scope SHA-256
488306d5e882292e7c67e9285ef1d28e68dfeaae7d61362d992802007d2f5298.

Require paired remediation/incident/export tests, fresh locked offline
all-targets, scoped formatting, checker 19/19, ratchet/strict inventories,
definition/SQL/DTO/test preservation, unchanged caller/neighbor hashes,
UTF-8/no-BOM checks, terminal Cargo cleanup and both Git diff checks. Every
new/extracted owner must remain at most 500 effective lines. Unit gates do not
establish live database, object-store, packaged runtime or release acceptance.

Serialize all Cargo operations, including desktop, and do not edit Rust during
a gate. S06 retains its implementation and cursor. GWP-20260912-01, runtime-profile
governance and final freeze/build transfer remain pending. No dependency,
baseline, exception, checker-policy, release or persistent deployment change
belongs to this lane. The overall plan remains open.

The entry is now 342 effective lines. Its 31 child source/test owners range from
91 to 473. Exact proof preserves 186 production items, 73 public paths, 48 public
DTOs, all private fields, 14 SQL literals, 21 original tests and four fixtures.
Forty-six private helpers gain pub(super); sixteen original test helper imports
are cfg-guarded. The four test groups retain exact bodies and leaf identities
with an explicit module-path mapping. All 488 neighboring inputs, including nine
Rust callers and eight additional contract files, are unchanged.

Paired remediation/incident/export tests pass 21/21, 6/6 and 12/12. Fresh
all-targets, scoped formatting, checker 19/19, ratchet, source/encoding/cleanup
proof and both Git checks pass. All Cargo gates are terminal; no process was
terminated. Strict scans 1,739 files: 21 hard, 27 mandatory and 40 soft. Forty-eight
remain above 700, giving 97/145 (66.9%) clearance. The S06 formatting pair remains.

Immutable scope SHA-256:
f23df37b01945aa7b572bb792a0ba3562bc90b1a687da73303e39b9737def063.
Report: ../../status/2026-09-13-remediation-owners.md. Access database ownership
is the next independent candidate; the overall plan and release remain open.
