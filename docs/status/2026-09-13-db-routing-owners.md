# Database-routing ownership

Accepted: 2026-09-13 02:55:25 UTC. Gateway-only structural checkpoint.

The routing entry decreases from 1,771 effective lines to 266. It retains all
13 concrete structs, their private SQL row fields and the Default implementation.
Seven private owners separate normalization, policies, aliases, model filters,
provider materialization, model catalog and candidate resolution. The existing
unit tests and fixtures move together. All ten scoped source/test files are at
most 266 effective lines.

| Source | Effective lines |
| --- | ---: |
| src/db/routing.rs | 266 |
| src/db/routing/normalization.rs | 243 |
| src/db/routing/policies.rs | 203 |
| src/db/routing/aliases.rs | 172 |
| src/db/routing/model_filters.rs | 176 |
| src/db/routing/providers.rs | 174 |
| src/db/routing/catalog.rs | 179 |
| src/db/routing/candidates.rs | 138 |
| src/db/routing/tests.rs | 259 |
| tests/python/test_gateway_phase3_access_reliability_contract.py | 75 |

Exact formatted-source proof preserves 54 production definitions, 19 public
entry paths, 17 SQL literals, 14 retained type/impl definitions, all nine original
unit tests and two fixtures. Only 14 cross-owner helper signatures gain
pub(super). A parent credential-view import preserves existing super-qualified
type references inside the moved bodies. There is no public visibility change.

The Python contract now reads alias persistence from routing/aliases.rs. Its
five test identities and 18 assertion sites, all other readers and assertions
are unchanged. Source proof freezes 396 neighboring inputs, including 13 caller
files. Every scoped source file is valid UTF-8 without BOM.

SQL text and bind order, transactions, access-projection version bumps, storage
fallback, project/model/provider filters, alias priorities, credential precedence,
timestamp/UUID creation and diagnostic behavior remain unchanged. The extraction
adds no resources, background work, blocking path or new allocation strategy.
Provider-payload diagnostic hardening requires a separate behavioral decision.

| Gate | Baseline | Final |
| --- | ---: | ---: |
| Database-routing unit tests | 9/9 | 9/9 |
| Rate-limit route_policy integration tests | 2/2 | 2/2 |
| Python access-reliability contract | 5/5 | 5/5 |

The paired Rust/Python test identities match; paired Rust warning sets are
unchanged. The rate-limit integration selection covers its two route_policy
tests and does not claim a live Redis test. Fresh locked offline all-targets,
scoped rustfmt, checker 19/19, ratchet, exact source/encoding proof, terminal Cargo
cleanup and both Git diff checks pass.

The first non-Cargo sequence stopped when cleanup observed live Cargo/rustc
processes. Its original logs and inventories remain intact; non-cargo-attempt1.json
records the failure and a later zero-process observation. The complete closing
sequence passed with attempt2 log names and individual receipts. No production
source or completed Cargo/test receipt was changed for that retry.

Global rustfmt still reports only the two unchanged S06 runtime-mirror files.
Strict scans 1,659 files: 26 hard, 27 mandatory and 40 soft. There are 53 files
above 700, down from 54; clearance is 92/145 (63.4%).

Immutable evidence:
target/effective-line-evidence/20260913-db-routing-owners/scope.json, SHA-256
23b72b61ab49a1b0aca97b1718704022922fde45b300bda7d01372b9499b4f75.

At source acceptance, Gateway HEAD is
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,842 status entries: 177 modified,
one unstaged deletion, two staged deletions and 1,662 untracked. Neuro HEAD is
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and 182
untracked. Inherited deletions and sibling work were preserved. Documentation
publication follows this immutable source snapshot.

Rate-limit-hotspot ownership is next. S06 retains its implementation/cursor;
GWP-20260912-01, runtime-profile governance and final freeze/build transfer remain
pending. No release, persistent deployment, dependency, baseline, exception or
checker-policy change was made. This checkpoint does not close the overall plan.
