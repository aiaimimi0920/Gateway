# Operator database ownership

Owner: resumed parallel coordinator. State: accepted at 2026-09-13 17:24:02 UTC.

Entry 2,810 -> 180 effective lines; all sixteen files are at most 414. Paired
operator/runtime/HTTP tests pass 9/11/10 with unchanged identities and warnings.
Fresh all-targets, scoped formatting, checker 19/19, ratchet and source/encoding/
cleanup/diff proof pass. Strict: 1,774 scanned, 19 hard, 27 mandatory, 40 soft;
46 remain above 700. [Acceptance report](../../status/2026-09-14-operator-owners.md).
Scope SHA-256: 294ea26375415d0223ef481aa51085ef1c3ed25027690b5a52b30fb37165e59c.
The cumulative accepted input union is 581. Database-root ownership is next.

Exclusive source scope: src/db/operator.rs and new private src/db/operator/
owners. The complete original implementation and nine tests have been read.
Preserve public paths, DTO fields/serde, SQL/binds, sorting, pricing precedence,
database-over-runtime identity priority, hidden inventory filters and Redis/DB
side-effect ordering. No behavior corrections belong to the structural phase.

Extract inventory, associations, cost overview/hints, runtime pressure/readiness,
probe state, breaker/runtime state, health, identity filters, usage queries,
pricing and pricing-editor owners. Public models remain stable through entry
reexports. Shared private records and CostProviderRef stay at the entry so no
private field or constructor needs broader visibility. Preserve all original
test bodies and module paths in one private test owner. Every result <=500.

Predecessor: access scope bf0ee20fe698631a88fdeb3f066aab444b7fb1dece6563215fb234bd1013d311;
carry its 556 current inputs forward. Evidence root:
target/effective-line-evidence/20260914-operator-owners/.

Pair operator units, provider-runtime caller units and operator-summary HTTP
contracts before/after. Require exact source/SQL/public/test proof, fresh locked
offline all-targets, scoped formatter, checker tests, ratchet/strict inventory,
UTF-8 without BOM, idle Cargo and separate Gateway/Neuro diff checks.
Unchecked pricing/count arithmetic and diagnostic text are separate leads.

All Cargo operations are serialized; no Rust edits while a gate runs. S06
ownership and GWP-20260912-01 remain reserved until an actual transfer receipt.
No dependencies, policy/baseline/exception changes, release or live deployment.
