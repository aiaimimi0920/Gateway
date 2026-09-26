# Database-routing ownership

Owner: parallel coordinator. State: verified. Started: 2026-09-13 UTC.

Accepted: 2026-09-13 02:55:25 UTC. The entry is 266 effective lines; all ten
scoped source/test files are at most 266. Exact proof preserves 54 production
definitions, 19 public paths, 17 SQL literals, 14 type/impl definitions, nine unit
tests, two fixtures, five Python tests/18 assertions and 396 neighbors. Only the
14 required helpers gain pub(super); the Python reader follows alias ownership.
Paired unit/integration/Python tests pass 9/9, 2/2 and 5/5 with identical identities
and unchanged Rust warnings. Fresh all-targets, scoped formatting, checker 19/19,
ratchet, source/encoding/cleanup proof and both Git checks pass. The first cleanup
attempt and successful attempt2 evidence are preserved. Strict: 1,659 scanned,
26 hard, 27 mandatory and 40 soft; 53 remain above 700, clearance 92/145 (63.4%).
Scope SHA-256: 23b72b61ab49a1b0aca97b1718704022922fde45b300bda7d01372b9499b4f75.
[Acceptance report](../../status/2026-09-13-db-routing-owners.md).

Scope: src/db/routing.rs, private db/routing/ owners and the existing Python
access-reliability contract's routing-source path. The entry has 1,771 effective
lines. Retain public DTOs, private SQL row types and Default at the entry; extract
normalization, policy persistence, alias persistence, model filtering, provider
materialization, model catalog and candidate resolution. Move the nine existing
unit tests and two fixtures together; every source owner must remain below 500.

The complete original source, public reexports, management normalization call and
rate-limit contract were inspected. Preserve public paths, fields/serde/defaults,
SQL text/bind order, transaction boundaries, access-projection version bumps,
project/model/provider filtering, alias priority, credential precedence, timestamp
and UUID creation, object-storage fallback, error text/codes and cancellation.
Keep private row fields at their original owner; widen only cross-owner helper
visibility to pub(super). Add one parent import so existing super-qualified
credential-view types continue to resolve without changing function bodies.

The Python access-reliability contract reads routing.rs directly. After moving
alias persistence, point that one read at routing/aliases.rs and preserve all
assertions and other readers. No fallback reader or duplicate implementation is
needed. Paired baseline/final gates: database-routing unit tests, the rate-limit
integration target's route_policy tests and all five Python contract tests.
Require fresh locked offline all-targets, scoped formatting, checker 19/19,
ratchet/strict inventories, exact definitions/SQL/public-path/fixture proof,
UTF-8/no-BOM checks, cleanup and both Git diff checks.

Evidence: target/effective-line-evidence/20260913-db-routing-owners/.
Serialize Cargo; do not edit Rust while a gate runs. Persistence semantics and
provider-payload diagnostics require separate decisions after structural proof.
S06 keeps its implementation/cursor; GWP-20260912-01, runtime-profile governance
and final freeze/build transfer remain pending. No dependency, baseline, exception,
checker-policy, release or persistent deployment change belongs here.
