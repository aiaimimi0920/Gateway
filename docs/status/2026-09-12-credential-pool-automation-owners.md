# Credential-pool automation ownership

Accepted: 2026-09-12 20:58:38 UTC. Gateway-only structural checkpoint.

src/credential_pool_automation.rs decreases from 1,521 to 247 effective lines.
Shared configuration, request/response and state contracts remain at the public
entry. Registry admission, runtime state, inventory transforms, archive storage,
provider actions, refill collection, reconciliation, driver execution and tests
now have explicit private owners.

| Owner under src/credential_pool_automation | Effective lines |
| --- | ---: |
| entry (src/credential_pool_automation.rs) | 247 |
| registry.rs | 163 |
| runtime.rs | 155 |
| inventory.rs | 84 |
| archive.rs | 177 |
| actions.rs | 144 |
| refill.rs | 75 |
| reconciliation.rs | 165 |
| driver.rs | 131 |
| tests.rs | 277 |

Exact source comparison preserves 59 production items: 31 free functions, 16
structs, three enums, five impl blocks and four constants. Fourteen public entry
paths and thirteen public runtime methods remain available. All field visibility,
serde attributes, configuration defaults, request/error text, provider lock
lifetime and archive-before-prune/commit order are unchanged. Seventeen helpers
and four private methods gain only family-local visibility. Test-only parent
imports remain cfg-gated.

All ten original test bodies and complete module paths are preserved, with no
replacement fixtures or weakened assertions. The paired automation suite passes
10/10, including its real HTTP driver request and archive/purge contracts. The
adjacent refill suite passes 7/7 before/after. All 286 neighboring inputs retain
their hashes, including management routes and their access checks.

Fresh serialized gates pass: cargo check --offline --locked --all-targets;
scoped formatter; checker 19/19; adoption ratchet; exact source/UTF-8/no-BOM proof;
and both repositories' git diff --check. All Cargo gates are terminal. Global
formatting still reports only the unchanged S06 runtime-mirror production/test
files. Strict scans 1,607 files: 29 hard, 29 mandatory and 40 soft; 58 remain above
700. Clearance is 87/145 (60.0%). No dependency, policy, baseline or exception changed.

Immutable evidence is target/effective-line-evidence/20260912-credential-pool-automation-owners/scope.json.
It binds baseline/candidate sources, paired test names/results, compiler/checker
logs, inventories and separate repository observations. At capture, Gateway has
1,774 dirty entries: 173 modified, one unstaged deletion, two staged deletions
and 1,598 untracked. Neuro has 192: ten modified and 182 untracked. Both HEADs
and inherited deletions remain unchanged. Historical accepted scopes stay immutable.

The driver currently checks its 2 MiB output limit after complete accumulation.
HTTP response bounds are the next focused regression. Script stdout/stderr bounds
and input timeout, archive filesystem races/blocking I/O, scheduler shutdown and
lock/state cardinality remain separate reviews. S06 ownership, GWP-20260912-01,
runtime-profile governance and final freeze/build transfer remain pending. No
immutable release, packaged runtime/UI acceptance or persistent deployment occurred.
