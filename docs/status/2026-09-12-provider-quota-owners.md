# Provider quota ownership

Accepted: 2026-09-12 17:28:52 UTC. Gateway-only structural checkpoint.

The provider quota entry decreases from 1,390 to 104 effective lines. Thirteen
files now separate cache storage, refresh orchestration/lock/timing, aggregation,
provider probes, HTTP contracts, payload lookup and the original test module.
All owners are at most 223 effective lines.

| Owner under src/provider_quota | Effective lines |
| --- | ---: |
| entry (src/provider_quota.rs) | 104 |
| cache.rs | 70 |
| refresh.rs | 154 |
| aggregation.rs | 164 |
| codex.rs | 149 |
| accio.rs | 133 |
| accio_http.rs | 167 |
| generic_balance.rs | 116 |
| payload_fields.rs | 73 |
| http_contract.rs | 33 |
| refresh_clock.rs | 30 |
| refresh_lock.rs | 61 |
| tests.rs | 223 |

Exact normalized comparison preserves 47 production items: 39 functions, seven
structs and one constant. Eleven public entry paths and the crate-local cached
refresh path remain unchanged. The entry retains public views and private Codex
wire records; field visibility is unchanged. Eighteen helpers and the private
refresh-lock type gain only family-local visibility. All seven original tests,
their complete module paths, two fixtures and 240 neighboring inputs are preserved.

The source was read completely before projection. Requests, headers, Accio
fallback/primary-error precedence, quota classification, cache keys/TTL, locking,
refresh/reset timing, aggregation tie/order behavior and serialization remain
unchanged. No new task, connection, allocation policy, credential handling or
dependency was introduced. The existing unbounded HTTP reads, transport-error URL
exposure, refresh-lock race and permissive numeric conversion remain explicit
hardening follow-ups; this structural result does not claim they are corrected.

Fresh default-feature verification passed: quota units 7/7 before and after;
`cargo check --offline --locked --all-targets`; scoped formatter; checker 19/19;
adoption ratchet; source/UTF-8/no-BOM proof; and both repositories' `git diff --check`.
All Cargo gates are terminal. Global formatting still reports only the unchanged
S06 runtime-mirror production and test files.

Strict scans 1,577 files: 31 hard, 30 mandatory and 40 soft; 61 remain above 700.
Accepted clearance is 84/145 (57.9%). No policy, baseline or exception changed.
Immutable evidence is `target/effective-line-evidence/20260912-provider-quota-owners/scope.json`.
It binds the source projection, frozen candidate, paired tests, compiler and
formatter/checker logs, inventories and repository observations.

At capture, Gateway has 1,735 dirty entries: 172 modified, one unstaged deletion,
two staged deletions and 1,560 untracked. Neuro has 192: ten modified and 182
untracked. Both HEADs and inherited deletions remain unchanged. Historical accepted
scopes are retained without regeneration.

The next bounded regression targets URL credentials in quota transport diagnostics,
using real loopback failures while retaining error classification and request wire
behavior. S06 ownership, pending GWP-20260912-01 and final freeze/build transfer are
unchanged. No immutable release, packaged runtime/UI acceptance or persistent
deployment was performed.
