# Access database ownership

Owner: parallel coordinator. State: structural_green, acceptance complete.

Accepted at 2026-09-13 16:40:19 UTC after recovery and fresh source-chain
verification. Entry 4,214 -> 307 effective lines; all 21 source/test files <=371.
Paired access/routing/key/management/isolated integration gates pass 6/9/5/12/3.
Final all-targets and scoped closing gates pass; strict remains 47 above 700.
The initial namespace collision and its import-only correction are preserved.
Report: [access acceptance](../../status/2026-09-14-access-owners.md).
Immutable scope SHA-256:
bf0ee20fe698631a88fdeb3f066aab444b7fb1dece6563215fb234bd1013d311.
Operator database ownership is next; no release is accepted by this lane.

Scope: src/db/access.rs and private access/ owners. The complete 4,214-effective-
line source, public/database reexports, 21 Rust consumers, desktop access
contracts, management admission tests and all nine original access tests were
read. Preserve all public interfaces and the crate-visible
bump_all_access_projection_versions path, transaction and monotonic SQL update.

Separate public models, row/view conversion, catalog reads/writes, bundle
mutations and OpenAI endpoint companions, key writes/rotation, balance runtime
and adjustments, projection queries/cache, authentication, membership boundaries,
sticky affinity, route filtering, candidate preview and route-context assembly.
Shared private records remain at the entry without widening field visibility.
Move the destructive database fixtures to a private test owner and retain the
original test names, bodies and ignore attributes.

Preserve SQL/binds, transaction and cache order, projection versions, provider
credential resolution, error behavior, ordering, serialization and resource
lifetime. Balance arithmetic/read-modify-write and save_access_key invalidation
observations remain outside this structural lane until separately reproduced.

Evidence: target/effective-line-evidence/20260913-access-owners/.
Original raw/canonical SHA-256:
6c42f2552773f7c507c7f248618767da4606fc0944571411d3b8aaccec8bfadc.
Predecessor: remediation ownership, scope SHA-256
f23df37b01945aa7b572bb792a0ba3562bc90b1a687da73303e39b9737def063.
Carry its 520-input accepted union forward with current accepted hashes.

Require paired access, routing, key-format and management/boundary contracts.
Run all three existing ignored PostgreSQL/Redis tests in freshly created
loopback-only containers using the same pinned local images before/after. The
fixture helpers issue DROP TABLE, so never pass inherited or live database URLs.
Clean up only identity-verified containers and their anonymous volumes.

Require fresh locked offline all-targets, scoped formatting, checker 19/19,
ratchet/strict inventories, exact definition/SQL/DTO/test preservation, unchanged
caller/neighbor hashes, UTF-8/no-BOM checks, terminal Cargo cleanup and both Git
diff checks. Every new/extracted owner must be at most 500 effective lines.

Serialize all Cargo operations, including desktop, and do not edit Rust during
a gate. S06 retains its implementation and cursor. GWP-20260912-01, runtime-profile
governance and final freeze/build transfer remain pending. No dependency,
baseline, exception, checker-policy, release or persistent deployment change
belongs to this lane. The overall plan remains open.
