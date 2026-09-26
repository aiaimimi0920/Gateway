# Credential stock and refill ownership

Accepted: 2026-09-12 19:47:40 UTC. Gateway-only structural checkpoint.

The stock/refill entries decrease from 1,514/1,306 to 281/263 effective lines.
Twenty Rust files separate public/wire records, policy and task persistence,
evaluation, signal and notification scheduling, claims, completion, delivery,
input validation and time formatting. Every owner remains at most 281 lines.

| Owner under src | Effective lines |
| --- | ---: |
| credential_stock.rs | 281 |
| credential_stock/evaluation.rs | 136 |
| credential_stock/policy.rs | 241 |
| credential_stock/status.rs | 235 |
| credential_stock/quota_fields.rs | 59 |
| credential_stock/signals.rs | 169 |
| credential_stock/signal_store.rs | 96 |
| credential_stock/validation.rs | 106 |
| credential_stock/tests.rs | 233 |
| credential_refill.rs | 263 |
| credential_refill/demand.rs | 156 |
| credential_refill/notifications.rs | 48 |
| credential_refill/creation.rs | 141 |
| credential_refill/claim.rs | 199 |
| credential_refill/completion.rs | 79 |
| credential_refill/delivery.rs | 152 |
| credential_refill/storage.rs | 136 |
| credential_refill/validation.rs | 80 |
| credential_refill/task_clock.rs | 14 |
| credential_refill/tests.rs | 123 |

Exact normalized comparison preserves 120 production items: 76 free functions,
32 types, ten constants and two complete implementation blocks. All 46 public
entry paths, serde attributes and field visibility remain unchanged. Thirty-eight
helpers gain only family-local visibility; test-only parent imports stay gated.
All 22 original Rust tests, their complete module paths and both fixtures remain
intact. Five raw stock SQL blocks and four refill Lua scripts are exact; all
remaining queries, bindings and execution order are preserved with their functions.

The initial Python wiring contract failed against the already-extracted router.
Its four old literal status labels also no longer occur in the current UI. The
separate source-contract correction reads the mounted route owner and the new
creation owner, and checks current endpoint/manual-refill component bindings.
The original trigger, endpoint, management-access and stream secret-exclusion
assertions remain unchanged. No UI production file changed. The corrected test
passes 1/1; its file measures 223 effective lines. This source test does not claim
rendered UI acceptance.

All 262 baseline neighboring inputs are unchanged. ProviderLifecycleBack and
ProviderStorageEndpoints are additionally hash-bound as the actual UI owners.
The original source was read completely before projection. State ownership,
database and Redis call order, token publication boundaries, three-attempt route
commit retries, credential deduplication, task leases and serialization remain
unchanged. No task, connection, allocation policy, credential delivery mode or
dependency was added. Cooldown overflow, signal publication bounds and broader
refill lease/delivery/notification lifecycle hardening remain separate work.

Fresh serialized gates pass: stock 15/15 and refill 7/7 before/after;
`cargo check --offline --locked --all-targets`; scoped formatter; checker 19/19;
adoption ratchet; source/UTF-8/no-BOM proof; focused Python 1/1; and both
repositories' `git diff --check`. All Cargo gates are terminal. Global formatting
still reports only the unchanged S06 runtime-mirror production/test files.
No PostgreSQL/Redis runtime or packaged UI gate is claimed for this extraction.

Strict scans 1,597 files: 30 hard, 29 mandatory and 40 soft; 59 remain above 700.
Clearance advances to 86/145 (59.3%). No policy, baseline or exception changed.
Immutable evidence: `target/effective-line-evidence/20260912-credential-stock-refill-owners/scope.json`.
It binds exact projections, paired Rust results, the separate Python correction,
compiler/checker logs, inventories and independent repository observations.

At capture, Gateway has 1,760 dirty entries: 173 modified, one unstaged deletion,
two staged deletions and 1,584 untracked. Neuro has 192: ten modified and 182
untracked. Both HEADs and inherited deletions remain unchanged. Historical
accepted scopes are retained without regeneration.

Stock signal cooldown arithmetic is the next bounded regression. S06 ownership,
GWP-20260912-01, runtime-profile governance and final freeze/build transfer remain
pending. No immutable release or persistent deployment was performed.
