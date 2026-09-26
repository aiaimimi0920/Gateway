# Console Redis storage ownership

Date: 2026-09-21. State: structural_green; full-goal work remains active.

Verified checkpoint: [`2026-09-21-console-redis-owners.md`](../../status/2026-09-21-console-redis-owners.md).
Parent 688 -> 440; types 159 and immutable JSON 104 effective lines. Paired
unit tests 7/7 and external transaction contracts 4/4 pass, with all-targets,
checker 19/19, ratchet, scoped formatting, Lua/source projection and Git checks.
Strict remains 28 above 700; soft entries decrease 39 -> 38. No release claimed.

Scope: `src/console/redis_store.rs` and new nested `types.rs` and
`immutable_json.rs` owners. The parent begins at 688 effective lines. Extract
validated key/revision/error/outcome types with canonical serialization, and
immutable JSON storage with its normalization Lua. Leave activation Lua,
transaction/session orchestration, response parsing and all existing tests in
the parent. Preserve private fields and existing public re-export paths.

Keep every function body and Lua byte string unchanged. Preserve validation
order, revision/digest identity checks, SET NX semantics, equivalent-JSON CAS
normalization, activation/PUBLISH ordering, exact errors and Cluster rejection.
Do not introduce new dependencies, exceptions or policy changes.

Evidence: `target/effective-line-evidence/20260921-console-redis-owners/`.
Run paired Redis-store unit tests and console transaction contracts, all-target
compilation, scoped formatter, exact formatted projection, checker tests/ratchet,
strict audit, encoding and separate Gateway/Neuro Git checks. All final owners
must be <=500 effective lines. Existing S06 source/build ownership and the full
repository completion requirements remain unchanged.
