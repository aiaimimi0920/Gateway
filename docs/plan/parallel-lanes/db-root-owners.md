# Database root ownership

Owner: resumed parallel coordinator. State: accepted at 2026-09-13 17:46:55 UTC.

All paired access/routing/management gates pass 6/9/12, with three unchanged
ignored access integration tests. All-targets, scoped formatting, checker 19/19,
ratchet, source/encoding/cleanup/diff proof pass. Strict: 1,780 scanned,
19 hard, 26 mandatory, 40 soft; 45 remain above 700. Accepted input union: 588.
Scope SHA-256: 3601f1b986685eb408e17bfae4f57b519cb65de036edf60fcd592483fa6efd4d.
[Acceptance report](../../status/2026-09-14-db-root-owners.md).

Frozen original: 1,423 effective lines, SHA-256
ddc2b4334b6074228465326f275ed14ff9977a723f209419ac1486170fd7161a.
Before snapshot contains 582 inputs and pins operator acceptance
294ea26375415d0223ef481aa51085ef1c3ed25027690b5a52b30fb37165e59c.
Projected entry is 417 lines; six owners are 186/235/96/217/167/140. Exact
proof preserves 52 definitions, 390 recognized public root paths, all original
reexport blocks (including the separate quota exports), 28 raw SQL literals,
private fields and 581 neighboring files. Five detail helpers gain pub(super).
Paired baseline gates pass access 6 (three ignored), routing 9 and management
12. Independent projection review found no concrete semantic/visibility issue.

Exclusive scope: src/db/mod.rs plus new src/db/account_models.rs,
project_api_access.rs, benefit_projects.rs, project_details.rs,
user_credentials.rs and provider_payloads.rs. The complete original root has
been read. All existing database module declarations and public reexports stay
stable. Shared private records and root helpers keep their original visibility.
Extract whole definitions with unchanged SQL, bind ordering, error messages,
credential checks, transaction boundaries and object-storage side effects.
Every resulting owner must be at most 500 effective lines.

The operator accepted input union is preserved. Pair existing access,
routing and management contracts, followed by locked offline all-targets,
scoped formatting, checker tests, ratchet/strict, exact source/SQL/API proof,
UTF-8 without BOM, idle process receipts and separate Gateway/Neuro diff checks.
There is no original root-local test module. Do not claim unrelated admission
tests prove database transaction behavior; unchanged body/SQL proof is required.

Separate review leads: nontransactional project-key rotation, unbounded credential
duration arithmetic and object-store/database write ordering are preserved by
this structural batch. No hidden behavioral correction belongs here.

All Cargo operations remain serialized; no Rust edits during compiler gates.
S06 remains reserved under GWP-20260912-01. No dependency, policy, baseline,
exception, release or persistent runtime changes belong to this batch.
