# Keepalive request dispatch and test ownership

Date: 2026-09-21. State: structural_green; release_pending.

Verified checkpoint:
[`2026-09-21-keepalive-dispatch-tests-owners.md`](../../status/2026-09-21-keepalive-dispatch-tests-owners.md).
Parent 2329 -> 1338 effective lines; eight new files are <=317. Paired
keepalive tests pass 24/24. All-target compilation, checker 19/19, ratchet,
scoped formatter, exact source projection, encoding and both Git checks pass.
The remaining parent is mandatory debt; the release reservation stays open.

The coordinator owns this incremental extraction from `src/keepalive.rs`:
wire request/response contracts, outbound steward dispatch, and existing tests
grouped by provider or header/browser-policy contract. New owners live under
`src/keepalive/`; each must remain below 500 effective lines. The parent starts
at 2329 effective lines. Do not change existing provider implementations,
shared reader behavior, S06 source scope, dependencies or line-count policy.

Preserve public `crate::keepalive` paths, serde attributes and field order,
all function/test bodies, header filtering, response merge order, timeout/error
behavior and the Suno API-key writeback exception. Keep fields visible only
inside `crate::keepalive`. Test functions retain their names and assertions;
only the module component of their fully qualified names changes.

Baseline/evidence: `target/effective-line-evidence/20260921-keepalive-dispatch-tests/`.
Run paired keepalive tests, all-target compilation, exact formatted projection,
official formatter, checker tests/ratchet, UTF-8/no-BOM and Git checks. Record
remaining root debt and separate strict-audit counts. The existing S06 build
reservation is unchanged; no release completion is implied by this increment.
