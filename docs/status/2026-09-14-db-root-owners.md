# Database root ownership acceptance

Accepted on 2026-09-13 at 17:46:55 UTC (2026-09-14 local time). The overall
Gateway refactor/release plan remains open.

## Scope and preserved contracts

The database entry src/db/mod.rs decreases from 1,423 to 417 effective lines.
Six private owners separate existing account contracts, project API access,
benefit provisioning, tenant/project detail reads, user credentials and provider
payload storage. The seven resulting files are all below 500:

| File under src/db/ | Effective lines | Responsibility |
| --- | ---: | --- |
| mod.rs | 417 | Stable database API and shared private SQL records |
| account_models.rs | 140 | Existing public account/credential contracts |
| project_api_access.rs | 186 | Project API-key lookup, creation and rotation |
| benefit_projects.rs | 235 | Benefit tenant/project and default routing transaction |
| project_details.rs | 96 | Tenant/project reads and view conversion |
| user_credentials.rs | 217 | Credential lookup, verification, issuance and revocation |
| provider_payloads.rs | 167 | Payload persistence and object-storage lifecycle |

Exact item-text proof preserves all 52 original definitions, all sixteen
existing reexport blocks, 390 public root names recognized by the path parser,
and the separate two quota exports covered by their unchanged reexport block.
All 28 r# SQL raw literals retain their hashes; full definition comparison also
preserves every other string/expression, query binding and operation order.
All original public database module declarations remain present.

Seven shared row types and their private fields stay at the root. The five root
helpers get_active_project, get_active_tenant, map_db_error, map_db_decode_error
and format_timestamp retain their original visibility and bodies. Only five
project-detail helpers gain pub(super). No public compatibility layer is added.
All 581 neighboring source/contract inputs are unchanged.

Review covered credential admission/expiry/scope checks, tenant/project lookup,
error translation, SQL binding, route-policy transaction ordering, object-store
operations, serialization and cleanup. Extraction adds no task, process, lock,
network operation or allocation policy. Existing nontransactional project-key
rotation, large credential-duration arithmetic and object-store/DB operation
ordering remain separate leads, with no hidden behavior correction.

## Verification

Paired unchanged caller tests pass with identical names, outcomes and warnings:
access 6/6 with three integration tests ignored by this unit command; routing
9/9; internal management/admission contracts 12/12 (4 plus 8). There is no
original root-local test module. These caller tests do not independently prove
database transaction behavior; exact full-body/SQL preservation is part of the
structural acceptance. No new transaction-coverage claim is made.

Locked offline all-targets compilation passed at 17:43:52.467 UTC. Fresh source
proof, scoped rustfmt, checker 19/19, ratchet, UTF-8 without BOM, idle process
proof and separate Gateway/Neuro diff checks passed. All Cargo gates ran
serially, all native handles are terminal, and no process was terminated.

Strict inventory: 1,780 scanned, 19 hard, 26 mandatory, 40 soft; 45 files remain
above 700. Clearance is 100/145 (69.0%). Strict exits 1; global rustfmt exits 1
only for the two unchanged reserved S06 runtime-mirror files.

Immutable evidence: target/effective-line-evidence/20260914-db-root-owners/scope.json.
SHA-256: 3601f1b986685eb408e17bfae4f57b519cb65de036edf60fcd592483fa6efd4d.
The accepted input union is 588 files. Acceptance verifies saved/current source,
snapshot and gate/log hashes, exact tests/ignore counts, serialized timing,
formatter findings, inventory and independent review.

At acceptance Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d:
1,987 entries (182 modified, one unstaged deletion, two staged deletions, 1,802
untracked). Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a:
192 entries (ten modified, 182 untracked). Inherited edits are preserved;
documentation publication adds its own changes after this census.

## Continuation

The coordinator has read build.rs and its 17 prebuilt-UI and six asset-serving
contracts. Build-time UI ownership is the next bounded source batch, using
serialized per-command prebuilt mode and preserved web-asset hashes. No build.rs
Rust change has yet been applied. S06 ownership/cursor transfer, runtime-profile
governance, final freeze/build transfer, full language/provider/release gates,
immutable packaging and packaged runtime/UI/Docker acceptance remain pending.
No release, dependency, policy, baseline, exception or persistent deployment
change occurred. The runtime acceptance target remains persistent 4200 and no
persistent 4226.
